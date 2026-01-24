//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier, Url};

use crate::{
    lean_rpc::RpcClient,
    tui_ipc::{CursorInfo, Position as TuiPosition, SocketServer},
};

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

        match rpc.get_goals(&text_document, position).await {
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
