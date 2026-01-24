//! TUI command processing with RPC lookups.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier, Url};

use crate::{
    lean_rpc::{GoToKind, RpcClient},
    tui_ipc::Command,
};

/// Process a command from TUI, potentially doing RPC lookups.
/// Returns a command to forward to the handler, or None if handled internally.
pub async fn process_command(cmd: Command, rpc_client: &Arc<RpcClient>) -> Option<Command> {
    match cmd {
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
