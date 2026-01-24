//! Goal fetching operations using Lean RPC.

use std::sync::Arc;

use crate::tui_ipc::{Broadcaster, CursorInfo, Position};

use super::RpcClient;

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
