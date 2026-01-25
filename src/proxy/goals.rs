//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier, Url};

use crate::{
    lean_rpc::RpcClient,
    tui_ipc::{CursorInfo, Position as TuiPosition, SocketServer},
};

/// Spawn a task to fetch goals and broadcast results or errors.
/// Fetches both tactic goals and term goal in parallel, prepending
/// term goal to the list if present.
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

        // Fetch tactic goals and term goal in parallel
        let (tactic_result, term_result) = tokio::join!(
            rpc.get_goals(&text_document, position),
            rpc.get_term_goal(&text_document, position)
        );

        // Combine results: term goal first, then tactic goals
        let mut all_goals = Vec::new();

        // Add term goal if present
        if let Ok(Some(term_goal)) = term_result {
            all_goals.push(term_goal);
        }

        // Add tactic goals
        match tactic_result {
            Ok(tactic_goals) => {
                all_goals.extend(tactic_goals);
            }
            Err(e) => {
                tracing::warn!("Could not fetch goals at {uri_string}:{line}:{character}: {e}");
                if all_goals.is_empty() {
                    socket_server.broadcast_error(e.to_string());
                    return;
                }
            }
        }

        socket_server.broadcast_goals(uri_string, TuiPosition { line, character }, all_goals);
    });
}
