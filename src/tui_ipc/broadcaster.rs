use std::{path::Path, sync::Arc};

use tokio::{io::AsyncWriteExt, net::UnixListener, sync::broadcast};

use super::message::{CursorInfo, Message, Position, SOCKET_PATH};
use crate::lake_ipc::Goal;

/// Broadcasts messages to connected TUI clients via Unix socket.
#[derive(Clone)]
pub struct Broadcaster {
    sender: broadcast::Sender<Message>,
}

impl Broadcaster {
    /// Create a new broadcaster.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(16);
        Self { sender }
    }

    /// Start accepting connections on the Unix socket.
    /// Spawns a background task that handles client connections.
    pub fn start_listener(self: Arc<Self>) {
        tokio::spawn(async move {
            // Remove existing socket file
            let path = Path::new(SOCKET_PATH);
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }

            let listener = match UnixListener::bind(SOCKET_PATH) {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to bind socket: {}", e);
                    return;
                }
            };

            tracing::info!("Listening on {}", SOCKET_PATH);

            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let mut rx = self.sender.subscribe();
                        tokio::spawn(async move {
                            let (_, mut writer) = stream.into_split();
                            while let Ok(msg) = rx.recv().await {
                                let Ok(json) = serde_json::to_string(&msg) else {
                                    continue;
                                };
                                let line = format!("{json}\n");
                                if writer.write_all(line.as_bytes()).await.is_err() {
                                    break;
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Accept error: {}", e);
                    }
                }
            }
        });
    }

    /// Broadcast a message to all connected clients.
    pub fn send(&self, msg: Message) {
        let _ = self.sender.send(msg);
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
