//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use crate::{
    lean_rpc::LeanDagClient,
    tui_ipc::{CursorInfo, SocketServer},
};

/// Spawn a task to fetch the proof DAG at the given cursor position.
///
/// Uses the dedicated `LeanDagClient` which manages its own LSP connection
/// to the lean-dag server, ensuring proper synchronization.
pub fn spawn_goal_fetch(
    cursor: &CursorInfo,
    socket_server: &Arc<SocketServer>,
    lean_dag_client: &Arc<LeanDagClient>,
) {
    let lean_dag_client = lean_dag_client.clone();
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

        // Fetch proof DAG using the dedicated lean-dag client
        // The client handles waitForDiagnostics internally
        let result = lean_dag_client.get_proof_dag(&uri, position, "tree").await;

        match result {
            Ok(Some(dag)) => {
                tracing::info!("ProofDag: {} nodes, root={:?}", dag.nodes.len(), dag.root);
                for node in &dag.nodes {
                    tracing::debug!(
                        "Node {} '{}': {} goals, {} hyps, new_hyps={:?}",
                        node.id,
                        node.tactic.text,
                        node.state_after.goals.len(),
                        node.state_after.hypotheses.len(),
                        node.new_hypotheses
                    );
                    for (i, g) in node.state_after.goals.iter().enumerate() {
                        tracing::debug!("  goal[{}]: name={:?}, type={}", i, g.username, g.type_);
                    }
                    for (i, h) in node.state_after.hypotheses.iter().enumerate() {
                        tracing::debug!("  hyp[{}]: name={}, type={}", i, h.name, h.type_);
                    }
                }
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
