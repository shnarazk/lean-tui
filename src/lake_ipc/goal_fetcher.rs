//! Goal fetching operations using Lean RPC.

use std::sync::Arc;

use super::RpcClient;
use crate::tui_ipc::{CursorInfo, Position, SocketServer};

/// Spawn a task to fetch goals and broadcast results or errors.
pub fn spawn_goal_fetch(
    cursor: &CursorInfo,
    socket_server: &Arc<SocketServer>,
    rpc_client: &Arc<RpcClient>,
) {
    let rpc = rpc_client.clone();
    let socket_server = socket_server.clone();
    let uri = cursor.uri.clone();
    let line = cursor.line();
    let character = cursor.character();

    tokio::spawn(async move {
        match rpc.get_goals(&uri, line, character).await {
            Ok(goals) => {
                socket_server.broadcast_goals(uri, Position { line, character }, goals);
            }
            Err(e) => {
                tracing::warn!("Could not fetch goals at {uri}:{line}:{character}: {e}");
                socket_server.broadcast_error(e);
            }
        }
    });
}
