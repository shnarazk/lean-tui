//! Handles commands from TUI clients (e.g., navigation requests).

use async_lsp::lsp_types::{Position, Range, ShowDocumentParams, Url};
use async_lsp::{ClientSocket, LanguageClient};
use tokio::sync::mpsc;

use super::Command;

/// Processes commands from TUI clients.
/// Currently handles navigation by sending `window/showDocument` to the editor.
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
        }
    }

    async fn send_show_document(&mut self, uri: &str, line: u32, character: u32) {
        let Ok(url) = Url::parse(uri) else {
            tracing::error!("Invalid URI: {uri}");
            return;
        };

        // LSP positions are 0-indexed
        let position = Position::new(line, character);
        let selection = Range::new(position, position);

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
