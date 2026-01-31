//! LSP proxy that sits between editor and lake serve.
//!
//! Architecture:
//! ```text
//! Editor ↔ [Service] ↔ lake serve
//!             ↓
//!        SocketServer → TUI clients
//! ```

mod cursor;
mod documents;
mod goals;
mod lake;
mod lsp;

use std::sync::{Arc, OnceLock};

use async_lsp::MainLoop;
use documents::DocumentCache;
use lake::spawn_lake_serve;
use lsp::{DeferredService, InterceptService, RpcClientSlot};
use tokio::io::{stdin, stdout};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::{
    error::{Error, LspError, Result},
    lean_rpc::RpcClient,
    tui_ipc::{CommandHandler, ServerMode, SocketServer},
};

/// Run the LSP proxy: editor ↔ lean-tui ↔ lake serve.
///
/// # Arguments
/// * `standalone` - If true, use the lean-dag binary (standalone mode).
///                  If false, use `lake serve` (library mode - users import LeanDag).
pub async fn run(standalone: bool) -> Result<()> {
    let server_mode = if standalone {
        tracing::info!("Running in standalone mode (lean-dag binary)");
        ServerMode::Standalone
    } else {
        tracing::info!("Running in library mode (lake serve + import LeanDag)");
        ServerMode::Library
    };

    let socket_server = Arc::new(SocketServer::new(server_mode));
    let document_cache = Arc::new(DocumentCache::new());

    // Create RPC client based on mode
    let rpc_client: Option<RpcClient> = match RpcClient::new(standalone).await {
        Ok(client) => {
            tracing::info!("RPC client initialized successfully");
            Some(client)
        }
        Err(e) => {
            tracing::warn!(
                "Failed to initialize RPC client: {}. Proof DAG will be unavailable.",
                e
            );
            None
        }
    };
    let rpc_client_slot: RpcClientSlot = Arc::new(OnceLock::new());
    if let Some(client) = rpc_client {
        let _ = rpc_client_slot.set(client);
    }

    // Spawn the editor-facing LSP server (lake serve)
    // Note: In both modes, the editor talks to lake serve for standard LSP.
    // The RPC client is separate and handles proof DAG fetching.
    let (child_stdin, child_stdout) = spawn_lake_serve()?;

    // Client-side: lean-tui → lake serve
    let doc_cache_client = document_cache.clone();
    let socket_server_client = socket_server.clone();
    let rpc_client_slot_client = rpc_client_slot.clone();
    let (mut client_mainloop, server_socket) = MainLoop::new_client(move |_| {
        InterceptService::new(
            DeferredService(None),
            socket_server_client.clone(),
            doc_cache_client,
            rpc_client_slot_client,
        )
    });

    // Server-side: editor → lean-tui
    let doc_cache_server = document_cache.clone();
    let socket_server_server = socket_server.clone();
    let rpc_client_slot_server = rpc_client_slot.clone();
    let (server_mainloop, client_socket) = MainLoop::new_server(move |_| {
        InterceptService::new(
            server_socket,
            socket_server_server.clone(),
            doc_cache_server,
            rpc_client_slot_server,
        )
    });

    // Start socket listener and get command receiver
    let cmd_rx = socket_server.start_listener();

    // Create command handler to process TUI commands
    let (cmd_handler, cmd_tx) = CommandHandler::new(client_socket.clone());

    // Forward commands from socket server to command handler
    tokio::spawn(async move {
        let mut cmd_rx = cmd_rx;
        while let Some(cmd) = cmd_rx.recv().await {
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
        server_mainloop
            .run_buffered(stdin().compat(), stdout().compat_write())
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
