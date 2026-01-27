//! TUI command processing with RPC lookups.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier, Url};

use super::{ast, goals::fetch_combined_goals, lsp::DocumentCache};
use crate::{
    lean_rpc::{GoToKind, RpcClient},
    tui_ipc::{Command, GoalResult, SocketServer, TemporalSlot},
};

/// Process a command from TUI, potentially doing RPC lookups.
/// Returns a command to forward to the handler, or None if handled internally.
pub async fn process_command(
    cmd: Command,
    rpc_client: &Arc<RpcClient>,
    document_cache: &Arc<DocumentCache>,
    socket_server: &Arc<SocketServer>,
) -> Option<Command> {
    match cmd {
        Command::FetchTemporalGoals {
            ref uri,
            cursor_position,
            slot,
        } => {
            handle_fetch_temporal_goals(
                uri,
                cursor_position,
                slot,
                rpc_client,
                document_cache,
                socket_server,
            )
            .await;
            None
        }
        Command::GetHypothesisLocation {
            ref uri,
            position,
            ref info,
        } => {
            let text_document = TextDocumentIdentifier::new(uri.clone());

            match rpc_client
                .get_goto_location(&text_document, position, GoToKind::Definition, info.clone())
                .await
            {
                Ok(Some(location)) => {
                    tracing::info!(
                        "Resolved hypothesis location to {}:{}:{}",
                        location.target_uri,
                        location.target_selection_range.start.line,
                        location.target_selection_range.start.character
                    );
                    Some(Command::Navigate {
                        uri: location.target_uri,
                        position: location.target_selection_range.start,
                    })
                }
                Ok(None) => {
                    tracing::info!("No definition found for hypothesis, using fallback");
                    Some(cmd)
                }
                Err(e) => {
                    tracing::error!("getGoToLocation failed: {e}");
                    Some(cmd)
                }
            }
        }
        Command::Navigate { .. } => Some(cmd),
    }
}

/// Handle `FetchTemporalGoals` command: find target position and fetch goals.
async fn handle_fetch_temporal_goals(
    uri: &Url,
    cursor_position: Position,
    slot: TemporalSlot,
    rpc_client: &Arc<RpcClient>,
    document_cache: &Arc<DocumentCache>,
    socket_server: &Arc<SocketServer>,
) {
    let uri_str = uri.as_str();
    let Some(tree) = document_cache.get_tree(uri_str) else {
        tracing::warn!("Document not in cache: {uri}");
        socket_server.broadcast_temporal_goals(
            uri.clone(),
            cursor_position,
            slot,
            GoalResult::NotAvailable,
        );
        return;
    };

    let target_position = match slot {
        TemporalSlot::Previous => ast::find_previous_tactic(&tree, cursor_position),
        TemporalSlot::Current => Some(cursor_position),
        TemporalSlot::Next => ast::find_next_tactic(&tree, cursor_position),
    };

    let Some(target) = target_position else {
        socket_server.broadcast_temporal_goals(
            uri.clone(),
            cursor_position,
            slot,
            GoalResult::NotAvailable,
        );
        return;
    };

    let text_document = TextDocumentIdentifier::new(uri.clone());

    match fetch_combined_goals(rpc_client, &text_document, target).await {
        Ok(goals) => {
            socket_server.broadcast_temporal_goals(
                uri.clone(),
                cursor_position,
                slot,
                GoalResult::Ready {
                    position: target,
                    goals,
                },
            );
        }
        Err(e) => {
            tracing::warn!(
                "Could not fetch temporal goals at {uri}:{}:{}: {e}",
                target.line,
                target.character
            );
            socket_server.broadcast_temporal_goals(
                uri.clone(),
                cursor_position,
                slot,
                GoalResult::Error {
                    error: e.to_string(),
                },
            );
        }
    }
}
