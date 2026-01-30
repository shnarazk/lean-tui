//! LSP proxy that sits between editor and lake serve.
//!
//! Architecture:
//! ```text
//! Editor ↔ [Service] ↔ lake serve
//!             ↓
//!        SocketServer → TUI clients
//! ```

pub mod ast;
mod commands;
mod goals;
mod lake;
mod lsp;

use std::sync::{Arc, OnceLock};

use async_lsp::MainLoop;
use commands::process_command;
use lake::spawn_lake_serve;
use lsp::{DeferredService, DocumentCache, InterceptService};
use tokio::io::{stdin, stdout};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::{
    error::{Error, LspError, Result},
    lean_rpc::{LeanDagClient, RpcClient},
    tui_ipc::{CommandHandler, SocketServer},
};

/// Run the LSP proxy: editor ↔ lean-tui ↔ lake serve.
pub async fn run() -> Result<()> {
    let socket_server = Arc::new(SocketServer::new());
    let document_cache = Arc::new(DocumentCache::new());

    // Create lean-dag client for proof DAG fetching
    let lean_dag_client = match LeanDagClient::new().await {
        Ok(client) => {
            tracing::info!("LeanDagClient initialized successfully");
            Some(client)
        }
        Err(e) => {
            tracing::warn!(
                "Failed to initialize LeanDagClient: {}. Proof DAG will be unavailable.",
                e
            );
            None
        }
    };
    let lean_dag_slot: Arc<OnceLock<Arc<LeanDagClient>>> = Arc::new(OnceLock::new());
    if let Some(client) = lean_dag_client {
        let _ = lean_dag_slot.set(client);
    }

    let (child_stdin, child_stdout) = spawn_lake_serve()?;

    // Shared slot for RPC client - set after LSP connection is established
    let rpc_client_slot: Arc<OnceLock<Arc<RpcClient>>> = Arc::new(OnceLock::new());

    // Client-side: lean-tui → lake serve
    let doc_cache_client = document_cache.clone();
    let socket_server_client = socket_server.clone();
    let lean_dag_slot_client = lean_dag_slot.clone();
    let (mut client_mainloop, server_socket) = MainLoop::new_client(move |_| {
        InterceptService::new(
            DeferredService(None),
            socket_server_client.clone(),
            doc_cache_client,
            lean_dag_slot_client,
        )
    });

    // Create RPC client from server socket and store in slot
    let rpc_client = Arc::new(RpcClient::new(server_socket.clone()));
    let _ = rpc_client_slot.set(rpc_client.clone());

    // Server-side: editor → lean-tui
    let doc_cache_server = document_cache.clone();
    let socket_server_server = socket_server.clone();
    let lean_dag_slot_server = lean_dag_slot.clone();
    let (server_mainloop, client_socket) = MainLoop::new_server(move |_| {
        InterceptService::new(
            server_socket,
            socket_server_server.clone(),
            doc_cache_server,
            lean_dag_slot_server,
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
