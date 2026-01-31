
use std::fs;

use async_lsp::{
    lsp_types::{Position, Range, ShowDocumentParams, Url},
    ClientSocket, LanguageClient,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{broadcast, mpsc},
};

use super::protocol::{socket_path, Command, CursorInfo, Message, ServerMode};
use crate::lean_rpc::ProofDag;


/// UNIX socket server that broadcasts messages to TUI clients.
pub struct LspProxySocketEndpoint {
    /// Sender for outgoing messages to TUI clients.
    msg_sender: broadcast::Sender<Message>,
    /// Server mode for RPC communication.
    server_mode: ServerMode,
}

impl LspProxySocketEndpoint {
    /// Create a new socket server with the specified server mode.
    pub fn new(server_mode: ServerMode) -> Self {
        let (msg_sender, _) = broadcast::channel(16);
        Self {
            msg_sender,
            server_mode,
        }
    }

    /// Start the socket listener.
    /// Returns a receiver for commands from TUI clients.
    pub fn start_listener(&self) -> mpsc::Receiver<Command> {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(16);
        let msg_sender = self.msg_sender.clone();
        let server_mode = self.server_mode;

        tokio::spawn(async move {
            run_listener(msg_sender, cmd_tx, server_mode).await;
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

    /// Broadcast proof dag to all connected clients.
    pub fn broadcast_proof_dag(
        &self,
        uri: Url,
        position: super::Position,
        proof_dag: Option<ProofDag>,
    ) {
        self.send(Message::ProofDag {
            uri,
            position,
            proof_dag,
        });
    }

    /// Broadcast error to all connected clients.
    pub fn broadcast_error(&self, error: String) {
        self.send(Message::Error { error });
    }
}

/// Run the UNIX socket listener.
async fn run_listener(
    msg_sender: broadcast::Sender<Message>,
    cmd_tx: mpsc::Sender<Command>,
    server_mode: ServerMode,
) {
    let path = socket_path();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Remove existing socket file
    if path.exists() {
        let _ = fs::remove_file(&path);
    }

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind socket at {}: {e}", path.display());
            return;
        }
    };

    tracing::info!("Listening on {}", path.display());

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let msg_rx = msg_sender.subscribe();
                let cmd_tx = cmd_tx.clone();
                tokio::spawn(handle_client(stream, msg_rx, cmd_tx, server_mode));
            }
            Err(e) => {
                tracing::error!("Accept error: {e}");
            }
        }
    }
}

/// Handle a single TUI client connection.
async fn handle_client(
    stream: UnixStream,
    mut msg_rx: broadcast::Receiver<Message>,
    cmd_tx: mpsc::Sender<Command>,
    server_mode: ServerMode,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // Send Connected message immediately with server mode
    if let Ok(json) = serde_json::to_string(&Message::Connected {
        server_mode: Some(server_mode),
    }) {
        let _ = writer.write_all(format!("{json}\n").as_bytes()).await;
    }

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

/// Processes commands from TUI clients.
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
            Command::Navigate { uri, position } => {
                tracing::info!(
                    "Navigate request: {uri}:{}:{}",
                    position.line,
                    position.character
                );
                self.send_show_document(uri, position).await;
            }
        }
    }

    async fn send_show_document(&mut self, uri: Url, position: Position) {
        let selection = Range::new(position, position);
        self.send_show_document_with_selection(uri, selection).await;
    }

    async fn send_show_document_with_selection(&mut self, uri: Url, selection: Range) {
        let params = ShowDocumentParams {
            uri,
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
