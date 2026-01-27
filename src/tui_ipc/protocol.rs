use std::path::PathBuf;

pub use async_lsp::lsp_types::{Position, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lean_rpc::{Goal, PaperproofOutputParams, PaperproofStep};
// Re-export AST-derived types from proxy for IPC consumers
pub use crate::proxy::ast::{CaseSplitInfo, DefinitionInfo, TacticInfo};

/// A unified proof step that can be populated from either Paperproof RPC
/// or local tree-sitter analysis.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProofStep {
    /// The tactic text.
    pub tactic: String,
    /// Position in source file.
    pub position: Position,
    /// Hypotheses this tactic depends on.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Nesting depth (for have/cases scopes).
    #[serde(default)]
    pub depth: usize,
    /// Source of this data (for debugging).
    #[serde(default)]
    pub source: ProofStepSource,
}

/// Source of proof step data.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProofStepSource {
    /// From Paperproof Lean library RPC.
    Paperproof,
    /// From local tree-sitter analysis.
    #[default]
    Local,
}

impl ProofStep {
    /// Create from Paperproof step data.
    pub fn from_paperproof(step: &PaperproofStep) -> Self {
        Self {
            tactic: step.tactic_string.clone(),
            position: step.position.start,
            depends_on: step.tactic_depends_on.clone(),
            depth: 0, // Paperproof doesn't provide depth
            source: ProofStepSource::Paperproof,
        }
    }

    /// Create from local tactic info.
    pub fn from_local(tactic: &TacticInfo, depends_on: Vec<String>) -> Self {
        Self {
            tactic: tactic.text.clone(),
            position: tactic.start,
            depends_on,
            depth: tactic.depth,
            source: ProofStepSource::Local,
        }
    }
}

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
        /// Case-splitting tactics that affect the current position.
        #[serde(default)]
        case_splits: Vec<CaseSplitInfo>,
        /// Proof steps from Paperproof if available.
        #[serde(default)]
        paperproof_steps: Option<Vec<PaperproofStep>>,
        /// Unified proof steps (from Paperproof or local analysis).
        #[serde(default)]
        proof_steps: Vec<ProofStep>,
        /// Index of current step (closest to cursor).
        #[serde(default)]
        current_step_index: usize,
    },
    TemporalGoals {
        uri: Url,
        cursor_position: Position,
        slot: TemporalSlot,
        result: GoalResult,
    },
    PaperproofData {
        uri: Url,
        position: Position,
        output: PaperproofOutputParams,
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
