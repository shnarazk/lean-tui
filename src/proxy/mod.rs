//! LSP proxy that sits between editor and lake serve.
//!
//! Architecture:
//! ```text
//! Editor ↔ [Service] ↔ lake serve
//!             ↓
//!        SocketServer → TUI clients
//! ```

mod commands;
mod cursor;
mod documents;
mod goals;
mod service;
mod tactic_finder;

use std::{process::Stdio, sync::Arc};

use async_lsp::MainLoop;
use commands::process_command;
use documents::DocumentCache;
use service::{DeferredService, InterceptService};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::{
    error::{Error, LspError, Result},
    lean_rpc::RpcClient,
    tui_ipc::{CommandHandler, SocketServer},
};

/// Run the LSP proxy: editor ↔ lean-tui ↔ lake serve.
pub async fn run() -> Result<()> {
    let socket_server = Arc::new(SocketServer::new());
    let document_cache = Arc::new(DocumentCache::new());

    let (child_stdin, child_stdout) = spawn_lake_serve()?;

    // Client-side: lean-tui → lake serve
    let doc_cache_client = document_cache.clone();
    let socket_server_client = socket_server.clone();
    let (mut client_mainloop, server_socket) = MainLoop::new_client(move |_| {
        InterceptService::with_document_cache(
            DeferredService(None),
            socket_server_client.clone(),
            None,
            doc_cache_client,
        )
    });

    // Create RPC client from server socket
    let rpc_client = Arc::new(RpcClient::new(server_socket.clone()));

    // Server-side: editor → lean-tui
    let doc_cache_server = document_cache.clone();
    let socket_server_server = socket_server.clone();
    let rpc_client_server = rpc_client.clone();
    let (server_mainloop, client_socket) = MainLoop::new_server(move |_| {
        InterceptService::with_document_cache(
            server_socket,
            socket_server_server.clone(),
            Some(rpc_client_server.clone()),
            doc_cache_server,
        )
    });

    // Start socket listener and get command receiver
    let cmd_rx = socket_server.start_listener();

    // Create command handler to process TUI commands
    let (cmd_handler, cmd_tx) = CommandHandler::new(client_socket.clone());

    // Forward commands from socket server to command handler,
    // intercepting GetHypothesisLocation for RPC lookup and FetchTemporalGoals
    let rpc_for_commands = rpc_client.clone();
    let doc_cache_for_commands = document_cache.clone();
    let socket_server_for_commands = socket_server.clone();
    tokio::spawn(async move {
        let mut cmd_rx = cmd_rx;
        while let Some(cmd) = cmd_rx.recv().await {
            let Some(cmd) = process_command(
                cmd,
                &rpc_for_commands,
                &doc_cache_for_commands,
                &socket_server_for_commands,
            )
            .await
            else {
                continue;
            };
            if cmd_tx.send(cmd).await.is_err() {
                break;
            }
        }
    });

    // Run command handler
    tokio::spawn(cmd_handler.run());

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
                .map_err(|e| Error::Lsp(LspError::RpcError { code: None, message: e.to_string() }))?
                .map_err(|e| Error::Lsp(LspError::RpcError { code: None, message: e.to_string() }))?;
        }
        result = server_task => {
            result
                .map_err(|e| Error::Lsp(LspError::RpcError { code: None, message: e.to_string() }))?
                .map_err(|e| Error::Lsp(LspError::RpcError { code: None, message: e.to_string() }))?;
        }
    }

    Ok(())
}

/// Lean pretty-printer options for the server.
const LEAN_PP_OPTIONS: &[&str] = &[
    "pp.showLetValues=true", // Show full let-binding values (not ⋯)
];

/// Spawn lake serve child process with configured Lean options.
fn spawn_lake_serve() -> Result<(tokio::process::ChildStdin, tokio::process::ChildStdout)> {
    let mut cmd = tokio::process::Command::new("lake");
    cmd.arg("serve").arg("--");
    for opt in LEAN_PP_OPTIONS {
        cmd.args(["-D", opt]);
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let stdin = child.stdin.take().ok_or_else(|| {
        Error::Lsp(LspError::RpcError {
            code: None,
            message: "Failed to capture lake serve stdin".to_string(),
        })
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        Error::Lsp(LspError::RpcError {
            code: None,
            message: "Failed to capture lake serve stdout".to_string(),
        })
    })?;

    Ok((stdin, stdout))
}
