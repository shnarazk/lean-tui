//! TUI command processing with RPC lookups.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier, Url};

use super::{documents::DocumentCache, goals::fetch_combined_goals, tactic_finder};
use crate::{
    lean_rpc::{GoToKind, RpcClient},
    tui_ipc::{Command, GoalResult, Position as TuiPosition, SocketServer, TemporalSlot},
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
            None // Handled internally, don't forward
        }
        Command::GetHypothesisLocation {
            ref uri,
            line,
            character,
            ref info,
        } => {
            // Use Lean's getGoToLocation RPC with the InfoWithCtx reference.
            let Ok(url) = Url::parse(uri) else {
                tracing::error!("Invalid URI: {uri}");
                return Some(cmd);
            };

            let text_document = TextDocumentIdentifier { uri: url };
            let position = Position::new(line, character);

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
                        uri: location.target_uri.to_string(),
                        line: location.target_selection_range.start.line,
                        character: location.target_selection_range.start.character,
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
    uri: &str,
    cursor_position: TuiPosition,
    slot: TemporalSlot,
    rpc_client: &Arc<RpcClient>,
    document_cache: &Arc<DocumentCache>,
    socket_server: &Arc<SocketServer>,
) {
    // Get parsed syntax tree for AST-based tactic finding (sync, uses cached tree)
    let Some(tree) = document_cache.get_tree(uri) else {
        tracing::warn!("Document not in cache: {uri}");
        socket_server.broadcast_temporal_goals(
            uri.to_string(),
            cursor_position,
            slot,
            GoalResult::NotAvailable,
        );
        return;
    };

    // Find target position based on slot using tree-sitter
    let target_position = match slot {
        TemporalSlot::Previous => tactic_finder::find_previous_tactic(&tree, cursor_position),
        TemporalSlot::Current => Some(cursor_position),
        TemporalSlot::Next => tactic_finder::find_next_tactic(&tree, cursor_position),
    };

    let Some(target) = target_position else {
        // No previous/next line found (at boundary)
        socket_server.broadcast_temporal_goals(
            uri.to_string(),
            cursor_position,
            slot,
            GoalResult::NotAvailable,
        );
        return;
    };

    // Fetch goals at target position
    let Ok(url) = Url::parse(uri) else {
        tracing::error!("Invalid URI: {uri}");
        socket_server.broadcast_temporal_goals(
            uri.to_string(),
            cursor_position,
            slot,
            GoalResult::Error {
                error: "Invalid URI".to_string(),
            },
        );
        return;
    };

    let text_document = TextDocumentIdentifier::new(url);
    let position = Position::new(target.line, target.character);

    match fetch_combined_goals(rpc_client, &text_document, position).await {
        Ok(goals) => {
            socket_server.broadcast_temporal_goals(
                uri.to_string(),
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
                uri.to_string(),
                cursor_position,
                slot,
                GoalResult::Error {
                    error: e.to_string(),
                },
            );
        }
    }
}
