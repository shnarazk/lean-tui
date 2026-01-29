//! Paperproof RPC types and integration.
//!
//! Calls Paperproof's `getSnapshotData` RPC method when the Paperproof library
//! is available in the user's Lean project. Falls back to CLI when RPC is
//! unavailable.
//!
//! See: <https://github.com/Paper-Proof/paperproof>

use std::process::Stdio;

use async_lsp::lsp_types::Position;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

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

/// Find the Lake project root by walking up from a file path.
fn find_lake_root(file_path: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(file_path);
    let mut dir = path.parent()?;

    loop {
        if dir.join("lakefile.lean").exists() || dir.join("lakefile.toml").exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// Call paperproof-cli to get proof steps when RPC is unavailable.
///
/// This is a fallback for when the user has `require Paperproof` in their lakefile
/// but hasn't added `import Paperproof` to their source file.
pub async fn fetch_paperproof_via_cli(
    file_path: &str,
    line: u32,
    column: u32,
    mode: PaperproofMode,
) -> Option<PaperproofOutputParams> {
    let project_root = find_lake_root(file_path)?;
    tracing::debug!("paperproof-cli: project root = {}", project_root.display());

    let mode_str = match mode {
        PaperproofMode::SingleTactic => "single_tactic",
        PaperproofMode::Tree => "tree",
    };

    let output = Command::new("lake")
        .current_dir(&project_root)
        .args([
            "exe",
            "paperproof-cli",
            "--by-position",
            file_path,
            &line.to_string(),
            &column.to_string(),
            mode_str,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // If paperproof-cli isn't available, that's expected - not an error
        if stderr.contains("unknown executable")
            || stderr.contains("no such file")
            || stderr.contains("not found")
        {
            tracing::debug!("paperproof-cli not available: {stderr}");
            return None;
        }
        tracing::warn!("paperproof-cli failed: {stderr}");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check for error response from CLI
    if let Ok(error_response) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(error) = error_response.get("error") {
            tracing::debug!("paperproof-cli returned error: {error}");
            return None;
        }
    }

    // Parse the output as PaperproofOutputParams
    match serde_json::from_str::<PaperproofOutputParams>(&stdout) {
        Ok(params) => Some(params),
        Err(e) => {
            tracing::warn!("Failed to parse paperproof-cli output: {e}");
            tracing::debug!("Raw output: {stdout}");
            None
        }
    }
}
