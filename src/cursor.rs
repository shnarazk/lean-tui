use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;
use tokio::sync::broadcast;

pub const SOCKET_PATH: &str = "/tmp/lean-tui.sock";

/// Position in a document (0-indexed line and character)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Cursor location with document URI and trigger method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CursorInfo {
    pub uri: String,
    pub position: Position,
    pub method: String,
}

impl CursorInfo {
    pub fn new(uri: String, line: u32, character: u32, method: &str) -> Self {
        Self {
            uri,
            position: Position { line, character },
            method: method.to_string(),
        }
    }

    /// Extract filename from URI for display
    pub fn filename(&self) -> &str {
        self.uri.rsplit('/').next().unwrap_or(&self.uri)
    }

    /// Convenience accessor for line
    pub fn line(&self) -> u32 {
        self.position.line
    }

    /// Convenience accessor for character
    pub fn character(&self) -> u32 {
        self.position.character
    }
}

/// Messages sent from proxy to TUI over the Unix socket.
/// Tagged enum for type-safe protocol extensibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    /// Cursor position update
    Cursor(CursorInfo),
    // Future: Goals { goals: Vec<Goal> }
    // Future: Diagnostics { diagnostics: Vec<Diagnostic> }
}

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
                    eprintln!("[lean-tui] Failed to bind socket: {}", e);
                    return;
                }
            };

            eprintln!("[lean-tui] Listening on {}", SOCKET_PATH);

            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let mut rx = self.sender.subscribe();
                        tokio::spawn(async move {
                            let (_, mut writer) = stream.into_split();
                            while let Ok(msg) = rx.recv().await {
                                let json = match serde_json::to_string(&msg) {
                                    Ok(j) => j,
                                    Err(_) => continue,
                                };
                                let line = format!("{}\n", json);
                                if writer.write_all(line.as_bytes()).await.is_err() {
                                    break;
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("[lean-tui] Accept error: {}", e);
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
}

/// Type alias for backwards compatibility
pub type CursorBroadcaster = Broadcaster;
