use std::path::PathBuf;

pub use async_lsp::lsp_types::{Position, Url};
use serde::{Deserialize, Serialize};

use crate::lean_rpc::ProofDag;

/// Returns the path to the Unix socket for IPC.
pub fn socket_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lean-tui/lean-tui.sock")
}

/// Cursor location with document URI and trigger method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorInfo {
    pub uri: Url,
    pub position: Position,
    pub method: String,
}

impl CursorInfo {
    pub fn new(uri: Url, position: Position, method: &str) -> Self {
        Self {
            uri,
            position,
            method: method.to_string(),
        }
    }

    pub fn filename(&self) -> Option<&str> {
        self.uri.path_segments()?.next_back()
    }
}

/// Server mode for RPC communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMode {
    /// Library mode: uses `lake serve`, requires `import LeanDag`
    Library,
    /// Standalone mode: uses lean-dag binary, no import required
    Standalone,
}

impl ServerMode {
    /// Display name for the server mode.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Standalone => "Standalone",
        }
    }
}

/// Messages sent from proxy to TUI over the Unix socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    Connected {
        #[serde(default)]
        server_mode: Option<ServerMode>,
    },
    Cursor(CursorInfo),
    /// Unified proof DAG - single source of truth for all display modes.
    ProofDag {
        uri: Url,
        position: Position,
        /// Contains all proof steps, tree structure, and state info.
        #[serde(default)]
        proof_dag: Option<ProofDag>,
    },
    Error {
        error: String,
    },
}

/// Commands sent from TUI to proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    Navigate { uri: Url, position: Position },
}
