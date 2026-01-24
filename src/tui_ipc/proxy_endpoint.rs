//! Proxy-side endpoint for Unix socket communication with TUI clients.
//!
//! This module runs in the proxy process and handles:
//! - Broadcasting messages (cursor position, goals) to connected TUI clients
//! - Processing commands from TUI clients (navigation requests)

use std::path::Path;

use async_lsp::{
    lsp_types::{Position, Range, ShowDocumentParams, Url},
    ClientSocket, LanguageClient,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
    sync::{broadcast, mpsc},
};

use super::protocol::{Command, CursorInfo, Message, SOCKET_PATH};
use crate::lean_rpc::Goal;

// ============================================================================
// SocketServer - broadcasts messages to TUI clients
// ============================================================================

/// Unix socket server that broadcasts messages to TUI clients.
/// Immutable after creation - all state is in channels.
pub struct SocketServer {
    /// Sender for outgoing messages to TUI clients.
    msg_sender: broadcast::Sender<Message>,
}

impl SocketServer {
    /// Create a new socket server.
    pub fn new() -> Self {
        let (msg_sender, _) = broadcast::channel(16);
        Self { msg_sender }
    }

    /// Start the socket listener.
    /// Returns a receiver for commands from TUI clients.
    pub fn start_listener(&self) -> mpsc::Receiver<Command> {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(16);
        let msg_sender = self.msg_sender.clone();

        tokio::spawn(async move {
            run_listener(msg_sender, cmd_tx).await;
        });

        cmd_rx
    }

    /// Broadcast a message to all connected clients.
    pub fn send(&self, msg: Message) {
        if self.msg_sender.send(msg).is_err() {
            tracing::debug!("No TUI clients connected to receive broadcast");
        }
    }

    /// Broadcast cursor info to all connected clients.
    pub fn broadcast_cursor(&self, cursor: CursorInfo) {
        self.send(Message::Cursor(cursor));
    }

    /// Broadcast goals to all connected clients.
    pub fn broadcast_goals(&self, uri: String, position: super::Position, goals: Vec<Goal>) {
        self.send(Message::Goals {
            uri,
            position,
            goals,
        });
    }

    /// Broadcast error to all connected clients.
    pub fn broadcast_error(&self, error: String) {
        self.send(Message::Error { error });
    }
}

/// Run the Unix socket listener.
async fn run_listener(msg_sender: broadcast::Sender<Message>, cmd_tx: mpsc::Sender<Command>) {
    // Remove existing socket file
    let path = Path::new(SOCKET_PATH);
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }

    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind socket: {e}");
            return;
        }
    };

    tracing::info!("Listening on {SOCKET_PATH}");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let msg_rx = msg_sender.subscribe();
                let cmd_tx = cmd_tx.clone();
                tokio::spawn(handle_client(stream, msg_rx, cmd_tx));
            }
            Err(e) => {
                tracing::error!("Accept error: {e}");
            }
        }
    }
}

/// Handle a single TUI client connection.
async fn handle_client(
    stream: tokio::net::UnixStream,
    mut msg_rx: broadcast::Receiver<Message>,
    cmd_tx: mpsc::Sender<Command>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    loop {
        tokio::select! {
            // Send messages to TUI
            msg_result = msg_rx.recv() => {
                let Ok(msg) = msg_result else { break };
                let json = match serde_json::to_string(&msg) {
                    Ok(j) => j,
                    Err(e) => {
                        tracing::warn!("Failed to serialize message: {e}");
                        continue;
                    }
                };
                if writer.write_all(format!("{json}\n").as_bytes()).await.is_err() {
                    break;
                }
            }
            // Receive commands from TUI
            line_result = lines.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        if let Ok(cmd) = serde_json::from_str::<Command>(&line) {
                            if cmd_tx.send(cmd).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }
}

// ============================================================================
// CommandHandler - processes commands from TUI clients
// ============================================================================

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
                ..
            } => {
                // Note: The actual RPC lookup using getGoToLocation happens in
                // the proxy module before the command reaches here. If we receive
                // this command, it means the lookup failed or returned the fallback.
                tracing::info!(
                    "GetHypothesisLocation fallback: navigating to {uri}:{line}:{character}"
                );
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
