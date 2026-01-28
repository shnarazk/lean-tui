//! Building `ProofDag` from Paperproof steps.

use std::collections::HashSet;

use async_lsp::lsp_types::Position;

use super::{
    node::{DagTacticInfo, ProofDagNode},
    state::ProofState,
    structure::build_tree_structure,
    NodeId, ProofDag, ProofDagSource,
};
use crate::lean_rpc::PaperproofStep;

impl ProofDag {
    /// Build a `ProofDag` from Paperproof steps.
    pub fn from_paperproof_steps(
        steps: &[PaperproofStep],
        cursor_position: Position,
        definition_name: Option<String>,
    ) -> Self {
        if steps.is_empty() {
            return Self::default();
        }

        let mut dag = Self {
            source: ProofDagSource::Paperproof,
            definition_name,
            initial_state: (&steps[0].goal_before).into(),
            nodes: steps
                .iter()
                .enumerate()
                .map(|(idx, step)| node_from_step(idx, step))
                .collect(),
            ..Default::default()
        };

        build_tree_structure(&mut dag, steps);
        dag.set_current_node(cursor_position);
        dag.root = (!dag.nodes.is_empty()).then_some(0);
        dag
    }
}

/// Build a single node from a Paperproof step.
fn node_from_step(idx: usize, step: &PaperproofStep) -> ProofDagNode {
    let state_before: ProofState = (&step.goal_before).into();
    let state_after = ProofState::from_goals_after(&step.goals_after);

    // Compute new hypotheses (diff)
    let before_ids: HashSet<&str> = step
        .goal_before
        .hyps
        .iter()
        .map(|h| h.id.as_str())
        .collect();

    let new_hypotheses: Vec<usize> = state_after
        .hypotheses
        .iter()
        .enumerate()
        .filter(|(_, h)| !before_ids.contains(h.id.as_str()))
        .map(|(i, _)| i)
        .collect();

    ProofDagNode {
        id: idx as NodeId,
        tactic: DagTacticInfo {
            text: step.tactic_string.clone(),
            depends_on: resolve_depends_on(step),
            theorems_used: step.theorems.iter().map(|t| t.name.clone()).collect(),
        },
        position: step.position.start,
        state_before,
        state_after,
        new_hypotheses,
        children: vec![],
        parent: None,
        depth: 0,
    }
}

/// Resolve fvar IDs in `tactic_depends_on` to user-visible hypothesis names.
fn resolve_depends_on(step: &PaperproofStep) -> Vec<String> {
    use std::collections::HashMap;

    let id_to_name: HashMap<&str, &str> = step
        .goal_before
        .hyps
        .iter()
        .map(|h| (h.id.as_str(), h.username.as_str()))
        .collect();

    step.tactic_depends_on
        .iter()
        .filter_map(|id| id_to_name.get(id.as_str()).map(|s| (*s).to_string()))
        .collect()
}
