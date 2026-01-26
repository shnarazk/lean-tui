//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier};

use crate::{
    error::LspError,
    lean_rpc::{Goal, RpcClient},
    tui_ipc::{CursorInfo, SocketServer},
};

/// Fetch tactic goals and term goal in parallel, combining them.
/// Term goal is prepended to the list if present.
pub async fn fetch_combined_goals(
    rpc_client: &RpcClient,
    text_document: &TextDocumentIdentifier,
    position: Position,
) -> Result<Vec<Goal>, LspError> {
    let (tactic_result, term_result) = tokio::join!(
        rpc_client.get_goals(text_document, position),
        rpc_client.get_term_goal(text_document, position)
    );

    let term_goal = term_result.ok().flatten();
    let tactic_goals = match tactic_result {
        Ok(goals) => goals,
        Err(e) if term_goal.is_none() => return Err(e),
        Err(_) => vec![],
    };

    Ok(term_goal.into_iter().chain(tactic_goals).collect())
}

/// Spawn a task to fetch goals and broadcast results or errors.
pub fn spawn_goal_fetch(
    cursor: &CursorInfo,
    socket_server: &Arc<SocketServer>,
    rpc_client: &Arc<RpcClient>,
) {
    let rpc = rpc_client.clone();
    let socket_server = socket_server.clone();
    let uri = cursor.uri.clone();
    let position = cursor.position;

    tokio::spawn(async move {
        let text_document = TextDocumentIdentifier::new(uri.clone());

        match fetch_combined_goals(&rpc, &text_document, position).await {
            Ok(goals) => {
                socket_server.broadcast_goals(uri, position, goals);
            }
            Err(e) => {
                tracing::warn!(
                    "Could not fetch goals at {uri}:{}:{}: {e}",
                    position.line,
                    position.character
                );
                socket_server.broadcast_error(e.to_string());
            }
        }
    });
}
