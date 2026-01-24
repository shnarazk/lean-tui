use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lake_ipc::Goal;

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
    pub const fn line(&self) -> u32 {
        self.position.line
    }

    /// Convenience accessor for character
    pub const fn character(&self) -> u32 {
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
    /// Proof goals at cursor position
    Goals {
        uri: String,
        position: Position,
        goals: Vec<Goal>,
    },
    /// Error message
    Error { error: String },
}

/// Commands sent from TUI to proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    /// Navigate to a location in the editor
    Navigate {
        uri: String,
        line: u32,
        character: u32,
    },
    /// Request go-to-definition location for a hypothesis
    /// The proxy will look up the location using `getGoToLocation` RPC
    /// and respond with a `Navigate` via `showDocument`
    GetHypothesisLocation {
        /// Document URI where the hypothesis appears
        uri: String,
        /// Line where goals were fetched (for RPC session)
        line: u32,
        /// Character position where goals were fetched
        character: u32,
        /// The `InfoWithCtx` reference from the hypothesis type's `SubexprInfo`
        info: Value,
    },
}
