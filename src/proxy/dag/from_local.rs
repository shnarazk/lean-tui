//! Building `ProofDag` from local tree-sitter tactics.

use async_lsp::lsp_types::Position;

use super::{
    node::{DagTacticInfo, ProofDagNode},
    state::ProofState,
    structure::build_local_tree_structure,
    NodeId, ProofDag, ProofDagSource,
};
use crate::{lean_rpc::Goal, proxy::ast::TacticInfo};

impl ProofDag {
    /// Build a `ProofDag` from local tactics (fallback when Paperproof
    /// unavailable).
    pub fn from_local_tactics(
        tactics: &[TacticInfo],
        goals: &[Goal],
        cursor_position: Position,
        definition_name: Option<String>,
    ) -> Self {
        if tactics.is_empty() {
            return Self::default();
        }

        let last_idx = tactics.len() - 1;
        let mut nodes: Vec<ProofDagNode> = tactics
            .iter()
            .enumerate()
            .map(|(idx, tactic)| node_from_tactic(idx, tactic, idx == last_idx))
            .collect();

        build_local_tree_structure(&mut nodes);

        let mut dag = Self {
            source: ProofDagSource::Local,
            definition_name,
            initial_state: ProofState::from_goals(goals),
            nodes,
            root: Some(0),
            ..Default::default()
        };

        dag.set_current_node(cursor_position);
        dag
    }
}

/// Build a single node from a local tactic (minimal info).
fn node_from_tactic(idx: usize, tactic: &TacticInfo, is_last: bool) -> ProofDagNode {
    ProofDagNode {
        id: idx as NodeId,
        tactic: DagTacticInfo {
            text: tactic.text.clone(),
            depends_on: extract_dependencies(&tactic.text),
            theorems_used: vec![],
        },
        position: tactic.start,
        state_before: ProofState::default(),
        state_after: ProofState::default(),
        new_hypotheses: vec![],
        changed_hypotheses: vec![],
        removed_hypotheses: vec![],
        children: vec![],
        parent: (idx > 0).then(|| (idx - 1) as NodeId),
        sibling_index: 0,
        sibling_count: 1,
        depth: tactic.depth,
        is_complete: false,
        is_leaf: is_last,
        is_current: false,
    }
}

/// Extract hypothesis names that appear in the tactic text.
fn extract_dependencies(tactic_text: &str) -> Vec<String> {
    tactic_text
        .split_whitespace()
        .skip(1) // Skip the tactic name
        .filter_map(|word| {
            let clean =
                word.trim_matches(|c| c == '[' || c == ']' || c == ',' || c == '⟨' || c == '⟩');

            if clean.is_empty()
                || clean.starts_with('-')
                || clean.starts_with('*')
                || matches!(clean, "with" | "at" | "only")
            {
                return None;
            }

            clean
                .chars()
                .next()
                .filter(|c| c.is_lowercase())
                .map(|_| clean.to_string())
        })
        .collect()
}
