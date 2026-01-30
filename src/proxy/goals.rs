//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use async_lsp::lsp_types::TextDocumentIdentifier;

use crate::{
    lean_rpc::RpcClient,
    tui_ipc::{CursorInfo, SocketServer},
};

pub fn spawn_goal_fetch(
    cursor: &CursorInfo,
    socket_server: &Arc<SocketServer>,
    rpc_client: &Arc<RpcClient>,
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

        let text_document = TextDocumentIdentifier { uri: uri.clone() };

        // Fetch pre-built ProofDag from LeanDag server
        let result = rpc_client
            .get_proof_dag(&text_document, position, "tree")
            .await;

        match result {
            Ok(Some(dag)) => {
                tracing::info!("ProofDag: {} nodes, root={:?}", dag.nodes.len(), dag.root);
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
