use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use crate::tui_ipc::{Message, SOCKET_PATH};

/// Spawn a background task that connects to the Unix socket and reads messages.
/// Returns a receiver channel for incoming messages.
pub fn spawn_socket_reader() -> mpsc::Receiver<Message> {
    let (tx, rx) = mpsc::channel::<Message>(16);

    tokio::spawn(async move {
        loop {
            match UnixStream::connect(SOCKET_PATH).await {
                Ok(stream) => {
                    let reader = BufReader::new(stream);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                            if tx.send(msg).await.is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(_) => {
                    // Retry connection after delay
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    rx
}
