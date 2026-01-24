//! TUI-side endpoint for Unix socket communication with the proxy.
//!
//! This module runs in the TUI process and handles:
//! - Connecting to the proxy's Unix socket
//! - Receiving messages (cursor position, goals) from the proxy
//! - Sending commands (navigation requests) to the proxy

use std::time::Duration;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    sync::mpsc,
};

use super::protocol::{Command, Message, SOCKET_PATH};

/// Handle for communicating with the proxy.
pub struct SocketHandle {
    /// Receiver for incoming messages from proxy.
    pub rx: mpsc::Receiver<Message>,
    /// Sender for outgoing commands to proxy.
    pub tx: mpsc::Sender<Command>,
}

/// Spawn a background task that connects to the Unix socket.
/// Returns a handle for bidirectional communication.
pub fn spawn_socket_handler() -> SocketHandle {
    let (msg_tx, msg_rx) = mpsc::channel::<Message>(16);
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(16);

    tokio::spawn(connection_loop(msg_tx, cmd_rx));

    SocketHandle {
        rx: msg_rx,
        tx: cmd_tx,
    }
}

async fn connection_loop(msg_tx: mpsc::Sender<Message>, mut cmd_rx: mpsc::Receiver<Command>) {
    loop {
        match UnixStream::connect(SOCKET_PATH).await {
            Ok(stream) => {
                handle_connection(stream, &msg_tx, &mut cmd_rx).await;
            }
            Err(_) => {
                // Retry connection after delay
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn handle_connection(
    stream: UnixStream,
    msg_tx: &mpsc::Sender<Message>,
    cmd_rx: &mut mpsc::Receiver<Command>,
) {
    let (reader, mut writer) = stream.into_split();
    let reader = BufReader::new(reader);
    let mut lines = reader.lines();

    loop {
        tokio::select! {
            line_result = lines.next_line() => {
                match handle_incoming_line(line_result, msg_tx).await {
                    Ok(true) => {}
                    Ok(false) | Err(()) => break,
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                if send_command(&mut writer, &cmd).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn handle_incoming_line(
    line_result: Result<Option<String>, std::io::Error>,
    msg_tx: &mpsc::Sender<Message>,
) -> Result<bool, ()> {
    match line_result {
        Ok(Some(line)) => {
            if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                if msg_tx.send(msg).await.is_err() {
                    return Err(()); // TUI closed, exit task
                }
            }
            Ok(true)
        }
        Ok(None) | Err(_) => Ok(false), // Connection closed, reconnect
    }
}

async fn send_command(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    cmd: &Command,
) -> Result<(), ()> {
    let Ok(json) = serde_json::to_string(cmd) else {
        return Ok(()); // Skip invalid command, don't break connection
    };
    let line = format!("{json}\n");
    writer.write_all(line.as_bytes()).await.map_err(|_| ())
}
