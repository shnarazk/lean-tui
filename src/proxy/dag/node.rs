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

    // === Tree structure (for DeductionTree mode) ===
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub depth: usize,

    /// True if this node has spawned goals (e.g., inline `by` blocks) that are not solved.
    #[serde(default)]
    pub has_unsolved_spawned_goals: bool,
}

impl ProofDagNode {
    /// All subgoals solved (`goals_after` empty).
    pub const fn is_complete(&self) -> bool {
        self.state_after.goals.is_empty()
    }

    /// No children in tree.
    pub const fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
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
