//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use crate::{
    lean_rpc::RpcClient,
    tui_ipc::{CursorInfo, SocketServer},
};

/// Spawn a task to fetch the proof DAG at the given cursor position.
///
/// Uses the RPC client which manages its own LSP connection,
/// ensuring proper synchronization.
pub fn spawn_goal_fetch(
    cursor: &CursorInfo,
    socket_server: &Arc<SocketServer>,
    rpc_client: &RpcClient,
) {
    let rpc_client = rpc_client.clone();
    let socket_server = socket_server.clone();
    let uri = cursor.uri.clone();
    let position = cursor.position;

    tokio::spawn(async move {
        tracing::info!(
            "Fetching proof DAG for {}:{}:{}",
            uri.path(),
            position.line,
            position.character
        );

        // Fetch proof DAG using the RPC client
        // The client handles waitForDiagnostics internally
        let result = rpc_client.get_proof_dag(&uri, position, "tree").await;

        match result {
            Ok(Some(dag)) => {
                tracing::debug!(
                    "ProofDag: {} nodes, root={:?}, current={:?}",
                    dag.nodes.len(),
                    dag.root,
                    dag.current_node
                );
                socket_server.broadcast_proof_dag(uri, position, Some(dag));
            }
            Ok(None) => {
                tracing::debug!("LeanDag.getProofDag returned no data at this position");
                socket_server.broadcast_proof_dag(uri, position, None);
            }
            Err(e) => {
                tracing::warn!(
                    "Could not fetch proof DAG at {uri}:{}:{}: {e}",
                    position.line,
                    position.character
                );
                socket_server.broadcast_error(e.to_string());
            }
        }
    });
}
