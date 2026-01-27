use std::path::PathBuf;

pub use async_lsp::lsp_types::{Position, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lean_rpc::Goal;
// Re-export AST-derived types from proxy for IPC consumers
pub use crate::proxy::ast::DefinitionInfo;
// Re-export ProofDag types
pub use crate::proxy::dag::{HypothesisInfo, NodeId, ProofDag, ProofDagNode, ProofState};

/// Returns the path to the Unix socket for IPC.
pub fn socket_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lean-tui/lean-tui.sock")
}

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

/// Messages sent from proxy to TUI over the Unix socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    Connected,
    Cursor(CursorInfo),
    Goals {
        uri: Url,
        position: Position,
        goals: Vec<Goal>,
        /// The enclosing definition (theorem, lemma, etc.)
        #[serde(default)]
        definition: Option<DefinitionInfo>,
        /// Unified proof DAG - single source of truth for all display modes.
        /// Contains all proof steps, tree structure, and navigation info.
        #[serde(default)]
        proof_dag: Option<ProofDag>,
    },
    TemporalGoals {
        uri: Url,
        cursor_position: Position,
        slot: TemporalSlot,
        result: GoalResult,
    },
    Error {
        error: String,
    },
}

/// Commands sent from TUI to proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    Navigate {
        uri: Url,
        position: Position,
    },
    GetHypothesisLocation {
        uri: Url,
        position: Position,
        info: Value,
    },
    FetchTemporalGoals {
        uri: Url,
        cursor_position: Position,
        slot: TemporalSlot,
    },
}
