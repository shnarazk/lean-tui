//! LSP proxy orchestration: spawning lake serve and managing bidirectional
//! communication.

use std::{process::Stdio, sync::Arc};

use async_lsp::MainLoop;
use tokio::process::Command;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use super::{forward::Forward, intercept::Intercept};
use crate::{
    error::{Error, Result},
    lake_ipc::RpcClient,
    tui_ipc::Broadcaster,
};

/// Spawn lake serve child process.
fn spawn_lake_serve() -> Result<(tokio::process::ChildStdin, tokio::process::ChildStdout)> {
    let mut child = Command::new("lake")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| Error::Lsp("Failed to capture lake serve stdin".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Lsp("Failed to capture lake serve stdout".to_string()))?;

    Ok((stdin, stdout))
}

/// Run the LSP proxy: editor ↔ lean-tui ↔ lake serve.
pub async fn run() -> Result<()> {
    let broadcaster = Arc::new(Broadcaster::new());
    broadcaster.clone().start_listener();

    let (child_stdin, child_stdout) = spawn_lake_serve()?;

    // Client-side: lean-tui → lake serve
    let (mut client_mainloop, server_socket) =
        MainLoop::new_client(|_| Intercept::new(Forward(None), broadcaster.clone(), None));

    // Create RPC client from server socket
    let rpc_client = Arc::new(RpcClient::new(server_socket.clone()));

    // Server-side: editor → lean-tui
    let (server_mainloop, client_socket) = MainLoop::new_server(|_| {
        Intercept::new(server_socket, broadcaster.clone(), Some(rpc_client.clone()))
    });

    // Link client side to server side
    client_mainloop.get_mut().service.0 = Some(client_socket);

    // Run both loops concurrently
    let client_task = tokio::spawn(async move {
        client_mainloop
            .run_buffered(child_stdout.compat(), child_stdin.compat_write())
            .await
    });

    let server_task = tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        server_mainloop
            .run_buffered(stdin.compat(), stdout.compat_write())
            .await
    });

    // Wait for either task to complete (or fail)
    tokio::select! {
        result = client_task => {
            result
                .map_err(|e| Error::Lsp(e.to_string()))?
                .map_err(|e| Error::Lsp(e.to_string()))?;
        }
        result = server_task => {
            result
                .map_err(|e| Error::Lsp(e.to_string()))?
                .map_err(|e| Error::Lsp(e.to_string()))?;
        }
    }

    Ok(())
}
