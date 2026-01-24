//! TUI command processing with RPC lookups.

use async_lsp::lsp_types::TextDocumentIdentifier;

use crate::{lean_rpc::RpcClient, tui_ipc::Command};

/// Process a command from TUI, potentially doing RPC lookups.
/// Returns a (possibly modified) command to forward to the handler.
pub async fn process_command(cmd: Command, rpc_client: &RpcClient) -> Command {
    match &cmd {
        Command::GetHypothesisLocation {
            uri,
            line,
            character,
            info: _,
        } => {
            // Use standard textDocument/definition LSP method.
            // This has access to the full InfoTree and can resolve binder locations
            // (where a hypothesis was introduced by `intro`, `fun`, etc.).
            let Ok(url) = async_lsp::lsp_types::Url::parse(uri) else {
                tracing::error!("Invalid URI: {uri}");
                return cmd;
            };

            let text_document = TextDocumentIdentifier { uri: url };
            let position = async_lsp::lsp_types::Position::new(*line, *character);

            match rpc_client.get_definition(&text_document, position).await {
                Ok(Some(location)) => {
                    tracing::info!(
                        "Resolved hypothesis location to {}:{}:{}",
                        location.target_uri,
                        location.target_selection_range.start.line,
                        location.target_selection_range.start.character
                    );
                    Command::Navigate {
                        uri: location.target_uri.to_string(),
                        line: location.target_selection_range.start.line,
                        character: location.target_selection_range.start.character,
                    }
                }
                Ok(None) => {
                    tracing::info!("No definition found for hypothesis, using fallback");
                    cmd
                }
                Err(e) => {
                    tracing::error!("textDocument/definition failed: {e}");
                    cmd
                }
            }
        }
        Command::Navigate { .. } => cmd,
    }
}
