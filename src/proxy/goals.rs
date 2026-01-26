//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier, Url};

use crate::{
    error::LspError,
    lean_rpc::{Goal, RpcClient},
    tui_ipc::{CursorInfo, Position as TuiPosition, SocketServer},
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

    let mut all_goals = Vec::new();

    if let Ok(Some(term_goal)) = term_result {
        all_goals.push(term_goal);
    }

    match tactic_result {
        Ok(tactic_goals) => all_goals.extend(tactic_goals),
        Err(e) if all_goals.is_empty() => return Err(e),
        Err(_) => {} // Term goal present, ignore tactic error
    }

    Ok(all_goals)
}

/// Spawn a task to fetch goals and broadcast results or errors.
pub fn spawn_goal_fetch(
    cursor: &CursorInfo,
    socket_server: &Arc<SocketServer>,
    rpc_client: &Arc<RpcClient>,
) {
    let rpc = rpc_client.clone();
    let socket_server = socket_server.clone();
    let uri_string = cursor.uri.clone();
    let line = cursor.line();
    let character = cursor.character();

    tokio::spawn(async move {
        let Ok(url) = Url::parse(&uri_string) else {
            tracing::error!("Invalid URI: {uri_string}");
            return;
        };
        let text_document = TextDocumentIdentifier::new(url);
        let position = Position::new(line, character);

        match fetch_combined_goals(&rpc, &text_document, position).await {
            Ok(goals) => {
                socket_server.broadcast_goals(uri_string, TuiPosition { line, character }, goals);
            }
            Err(e) => {
                tracing::warn!("Could not fetch goals at {uri_string}:{line}:{character}: {e}");
                socket_server.broadcast_error(e.to_string());
            }
        }
    });
}
