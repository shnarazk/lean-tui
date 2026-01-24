//! Unix socket server that broadcasts messages to connected TUI clients.

use std::path::Path;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
    sync::{broadcast, mpsc},
};

use super::protocol::{Command, CursorInfo, Message, Position, SOCKET_PATH};
use crate::lake_ipc::Goal;

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
        let _ = self.msg_sender.send(msg);
    }

    /// Broadcast cursor info to all connected clients.
    pub fn broadcast_cursor(&self, cursor: CursorInfo) {
        self.send(Message::Cursor(cursor));
    }

    /// Broadcast goals to all connected clients.
    pub fn broadcast_goals(&self, uri: String, position: Position, goals: Vec<Goal>) {
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
                let Ok(json) = serde_json::to_string(&msg) else { continue };
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
