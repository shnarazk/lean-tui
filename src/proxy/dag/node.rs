//! Proof DAG node - represents a tactic application and its effects.

use async_lsp::lsp_types::Position;
use serde::{Deserialize, Serialize};

use super::{state::ProofState, NodeId};

/// A node representing a proof state after applying a tactic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofDagNode {
    pub id: NodeId,

    // === Tactic information ===
    pub tactic: DagTacticInfo,
    pub position: Position,

    // === State before/after (for BeforeAfter mode) ===
    pub state_before: ProofState,
    pub state_after: ProofState,

    // === Diff information (pre-computed) ===
    /// Indices into `state_after.hypotheses` for new hypotheses.
    pub new_hypotheses: Vec<usize>,
    /// Indices for hypotheses with changed types.
    pub changed_hypotheses: Vec<usize>,
    /// Names of removed hypotheses.
    pub removed_hypotheses: Vec<String>,

    // === Tree structure (for DeductionTree mode) ===
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub sibling_index: usize,
    pub sibling_count: usize,
    pub depth: usize,

    // === Status flags ===
    /// All subgoals solved (`goals_after` empty).
    pub is_complete: bool,
    /// No children in tree.
    pub is_leaf: bool,
    /// This is the current node (closest to cursor).
    pub is_current: bool,
}

/// Information about a tactic application.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DagTacticInfo {
    /// The tactic text (e.g., "intro n").
    pub text: String,
    /// Hypothesis names used by this tactic.
    pub depends_on: Vec<String>,
    /// Theorem names referenced by this tactic.
    pub theorems_used: Vec<String>,
}
