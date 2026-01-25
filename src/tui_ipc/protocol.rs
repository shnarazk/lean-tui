use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lean_rpc::Goal;

pub const SOCKET_PATH: &str = "/tmp/lean-tui.sock";

/// Which temporal slot a goal state belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemporalSlot {
    /// Goals before the last tactic (at previous line)
    Previous,
    /// Goals at current cursor position
    Current,
    /// Goals after the next tactic (at next line)
    Next,
}

/// Result of fetching goals for a temporal slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum GoalResult {
    /// Successfully fetched goals at the position.
    Ready {
        position: Position,
        goals: Vec<Goal>,
    },
    /// No goals available (at proof boundary or outside proof).
    NotAvailable,
    /// Error fetching goals.
    Error { error: String },
}

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
    /// Proof goals at cursor position (legacy, kept for compatibility)
    Goals {
        uri: String,
        position: Position,
        goals: Vec<Goal>,
    },
    /// Goals for a specific temporal slot (previous/current/next)
    TemporalGoals {
        uri: String,
        /// The cursor position these goals are relative to
        cursor_position: Position,
        /// Which temporal slot this is
        slot: TemporalSlot,
        /// The result of fetching goals
        result: GoalResult,
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
    /// Request goals for a temporal slot (previous/next line)
    FetchTemporalGoals {
        /// Document URI
        uri: String,
        /// Current cursor position
        cursor_position: Position,
        /// Which slot to fetch
        slot: TemporalSlot,
    },
}
