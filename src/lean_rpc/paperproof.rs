//! Paperproof RPC types and integration.
//!
//! Calls Paperproof's `getSnapshotData` RPC method when the Paperproof library
//! is available in the user's Lean project.
//!
//! See: <https://github.com/Paper-Proof/paperproof>

use async_lsp::lsp_types::Position;
use serde::{Deserialize, Serialize};

/// Input parameters for Paperproof's `getSnapshotData` RPC method.
#[derive(Debug, Clone, Serialize)]
pub struct PaperproofInputParams {
    pub pos: Position,
    pub mode: PaperproofMode,
}

/// Mode for Paperproof analysis.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaperproofMode {
    /// Single tactic analysis (faster).
    #[default]
    SingleTactic,
    /// Full proof tree analysis.
    Tree,
}

/// Output from Paperproof's `getSnapshotData` RPC method.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaperproofOutputParams {
    pub steps: Vec<PaperproofStep>,
    pub version: u32,
}

/// A single proof step from Paperproof.
///
/// Based on Paperproof's `Services.ProofStep` structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaperproofStep {
    /// The tactic text (e.g., "intro n", "apply h").
    pub tactic_string: String,
    /// Goal state before this tactic was applied.
    pub goal_before: PaperproofGoalInfo,
    /// Goal states after this tactic was applied.
    pub goals_after: Vec<PaperproofGoalInfo>,
    /// Hypothesis names this tactic depends on.
    pub tactic_depends_on: Vec<String>,
    /// Goals spawned by this tactic (for `have`, `cases`, etc.).
    pub spawned_goals: Vec<PaperproofGoalInfo>,
    /// Position in source file.
    pub position: PaperproofStepPosition,
    /// Theorems used by this tactic.
    #[serde(default)]
    pub theorems: Vec<PaperproofTheoremSignature>,
}

/// Goal information from Paperproof.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaperproofGoalInfo {
    /// User-visible name for the goal.
    pub username: String,
    /// Goal type as a string.
    #[serde(rename = "type")]
    pub type_: String,
    /// Hypotheses in scope.
    pub hyps: Vec<PaperproofHypothesis>,
    /// Internal goal ID.
    #[serde(default)]
    pub id: String,
}

/// Hypothesis from Paperproof.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaperproofHypothesis {
    /// User-visible name.
    pub username: String,
    /// Type as a string.
    #[serde(rename = "type")]
    pub type_: String,
    /// Value for let-bindings.
    #[serde(default)]
    pub value: Option<String>,
    /// Internal hypothesis ID.
    #[serde(default)]
    pub id: String,
    /// Whether this hypothesis is a proof.
    #[serde(default)]
    pub is_proof: String,
}

/// Position range for a proof step.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaperproofStepPosition {
    pub start: Position,
    pub stop: Position,
}

/// Theorem signature used by a tactic.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaperproofTheoremSignature {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub signature: String,
}

/// RPC method name for Paperproof.
pub const PAPERPROOF_GET_SNAPSHOT_DATA: &str = "Paperproof.getSnapshotData";
