//! Handles commands from TUI clients (e.g., navigation requests).
//!
//! This module runs in the proxy process and processes commands received
//! from TUI clients over the Unix socket, typically by sending LSP requests
//! to the editor (e.g., `window/showDocument`).

use async_lsp::lsp_types::{Position, Range, ShowDocumentParams, Url};
use async_lsp::{ClientSocket, LanguageClient};
use serde_json::Value;
use tokio::sync::mpsc;

use super::Command;

/// Processes commands from TUI clients.
/// Runs in the proxy process and sends LSP requests to the editor.
pub struct CommandHandler {
    /// Channel to receive commands from TUI clients.
    rx: mpsc::Receiver<Command>,
    /// Socket to send LSP requests to the editor.
    socket: ClientSocket,
}

impl CommandHandler {
    /// Create a new command handler and return a sender for commands.
    pub fn new(socket: ClientSocket) -> (Self, mpsc::Sender<Command>) {
        let (tx, rx) = mpsc::channel(16);
        let handler = Self { rx, socket };
        (handler, tx)
    }

    /// Run the command handler, processing commands until the channel closes.
    pub async fn run(mut self) {
        while let Some(cmd) = self.rx.recv().await {
            self.handle_command(cmd).await;
        }
    }

    async fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::Navigate {
                uri,
                line,
                character,
            } => {
                tracing::info!("Navigate request: {uri}:{line}:{character}");
                self.send_show_document(&uri, line, character).await;
            }
            Command::GetHypothesisLocation {
                uri,
                line,
                character,
                info,
            } => {
                tracing::info!("GetHypothesisLocation request for {uri}");
                self.handle_get_hypothesis_location(&uri, line, character, info)
                    .await;
            }
        }
    }

    async fn handle_get_hypothesis_location(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        _info: Value,
    ) {
        // NOTE: We intentionally don't use `getGoToLocation` with the hypothesis type's info
        // because that navigates to the TYPE definition (e.g., `Eq` in Prelude.lean) rather than
        // where the hypothesis is introduced. Lean's RPC API doesn't expose hypothesis binder
        // locations directly - that would require InfoTree access.
        //
        // For now, navigate to the cursor position (tactic block) as a reasonable fallback.
        // This is better than navigating to the wrong place (type definition).
        //
        // Future: Could use LSP textDocument/definition at the hypothesis name position in source,
        // or implement a custom RPC that searches the InfoTree for the binder.
        tracing::info!(
            "Hypothesis navigation: falling back to tactic position (type info would go to type definition)"
        );
        self.send_show_document(uri, line, character).await;
    }

    async fn send_show_document(&mut self, uri: &str, line: u32, character: u32) {
        let Ok(url) = Url::parse(uri) else {
            tracing::error!("Invalid URI: {uri}");
            return;
        };

        // LSP positions are 0-indexed
        let position = Position::new(line, character);
        let selection = Range::new(position, position);

        self.send_show_document_with_url(url, selection).await;
    }

    async fn send_show_document_with_url(&mut self, url: Url, selection: Range) {
        let params = ShowDocumentParams {
            uri: url,
            external: None,
            take_focus: Some(true),
            selection: Some(selection),
        };

        tracing::debug!("showDocument params: {params:?}");

        match self.socket.show_document(params).await {
            Ok(result) => {
                tracing::info!("showDocument result: success={}", result.success);
            }
            Err(e) => {
                tracing::error!("showDocument failed: {e:?}");
            }
        }
    }
}
