//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use crate::{
    lake_ipc::RpcClient,
    tui_ipc::{Broadcaster, CursorInfo, Position},
};

/// Spawn a task to fetch goals and broadcast results or errors.
pub fn spawn_goal_fetch(
    cursor: &CursorInfo,
    broadcaster: &Arc<Broadcaster>,
    rpc_client: &Arc<RpcClient>,
) {
    let rpc = rpc_client.clone();
    let broadcaster = broadcaster.clone();
    let uri = cursor.uri.clone();
    let line = cursor.line();
    let character = cursor.character();

    tokio::spawn(async move {
        match rpc.get_goals(&uri, line, character).await {
            Ok(goals) => {
                broadcaster.broadcast_goals(uri, Position { line, character }, goals);
            }
            Err(e) => {
                tracing::error!("Failed to get goals: {e}");
                broadcaster.broadcast_error(e);
            }
        }
    });
}

/// Handle cursor position: broadcast it and optionally fetch goals.
pub fn handle_cursor_and_goals(
    cursor: &CursorInfo,
    broadcaster: &Arc<Broadcaster>,
    rpc_client: Option<&Arc<RpcClient>>,
) {
    let _span = tracing::info_span!(
        "cursor",
        file = cursor.filename(),
        line = cursor.line(),
        char = cursor.character()
    )
    .entered();
    tracing::info!("cursor position");

    broadcaster.broadcast_cursor(cursor.clone());

    if let Some(rpc) = rpc_client {
        spawn_goal_fetch(cursor, broadcaster, rpc);
    }
}
