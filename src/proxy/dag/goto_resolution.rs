//! Goto location resolution for `ProofDag` nodes.
//!
//! Enriches Paperproof data with goto locations by fetching interactive goals
//! at each step position and matching by hypothesis/goal name.

use std::collections::HashMap;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier};

use super::{node::ProofDagNode, state::HypothesisInfo, ProofDag};
use crate::lean_rpc::{Goal, GotoLocations, RpcClient};

type PositionKey = (u32, u32);
type LocationMap = HashMap<String, GotoLocations>;

impl ProofDag {
    /// Resolve goto locations by fetching interactive goals at each step
    /// position.
    pub async fn resolve_goto_locations(&mut self, rpc: &RpcClient, doc: &TextDocumentIdentifier) {
        let positions = self.collect_positions();

        for (pos_key, indices) in positions {
            let goals = fetch_resolved_goals(rpc, doc, pos_key).await;
            apply_to_nodes(&mut self.nodes, &indices, &goals);
        }

        self.resolve_initial_state(rpc, doc).await;
    }

    fn collect_positions(&self) -> HashMap<PositionKey, Vec<usize>> {
        self.nodes
            .iter()
            .enumerate()
            .fold(HashMap::new(), |mut acc, (i, n)| {
                acc.entry((n.position.line, n.position.character))
                    .or_default()
                    .push(i);
                acc
            })
    }

    async fn resolve_initial_state(&mut self, rpc: &RpcClient, doc: &TextDocumentIdentifier) {
        let Some(pos) = self.nodes.first().map(|n| n.position) else {
            return;
        };

        let goals = fetch_resolved_goals(rpc, doc, (pos.line, pos.character)).await;
        let hyp_locs = hyp_locations(&goals);

        self.initial_state
            .hypotheses
            .iter_mut()
            .for_each(|h| apply_hyp_location(h, &hyp_locs));

        if let (Some(g), Some(ig)) = (goals.first(), self.initial_state.goals.first_mut()) {
            ig.goto_locations = g.goto_locations.clone();
        }
    }
}

async fn fetch_resolved_goals(
    rpc: &RpcClient,
    doc: &TextDocumentIdentifier,
    (line, character): PositionKey,
) -> Vec<Goal> {
    let pos = Position { line, character };
    let Ok(mut goals) = rpc.get_goals(doc, pos).await else {
        return vec![];
    };

    for goal in &mut goals {
        rpc.resolve_goto_locations(doc, pos, goal).await;
    }

    goals
}

fn hyp_locations(goals: &[Goal]) -> LocationMap {
    goals
        .iter()
        .flat_map(|g| &g.hyps)
        .flat_map(|h| {
            h.names
                .iter()
                .map(|n| (n.clone(), h.goto_locations.clone()))
        })
        .collect()
}

fn goal_locations(goals: &[Goal]) -> LocationMap {
    goals
        .iter()
        .filter_map(|g| {
            g.user_name
                .as_ref()
                .map(|n| (n.clone(), g.goto_locations.clone()))
        })
        .collect()
}

fn apply_to_nodes(nodes: &mut [ProofDagNode], indices: &[usize], goals: &[Goal]) {
    if goals.is_empty() {
        return;
    }

    let hyp_locs = hyp_locations(goals);
    let goal_locs = goal_locations(goals);

    for &i in indices {
        let Some(node) = nodes.get_mut(i) else {
            continue;
        };
        apply_locations(&mut node.state_after, &hyp_locs, &goal_locs);
    }
}

fn apply_locations(
    state: &mut super::state::ProofState,
    hyp_locs: &LocationMap,
    goal_locs: &LocationMap,
) {
    for h in &mut state.hypotheses {
        if let Some(l) = hyp_locs.get(&h.name) {
            tracing::trace!("Resolved goto for hypothesis '{}': {:?}", h.name, l);
            h.goto_locations = l.clone();
        }
    }
    for g in &mut state.goals {
        if let Some(l) = goal_locs.get(&g.username) {
            tracing::trace!("Resolved goto for goal '{}': {:?}", g.username, l);
            g.goto_locations = l.clone();
        }
    }
}

fn apply_hyp_location(hyp: &mut HypothesisInfo, locs: &LocationMap) {
    if let Some(l) = locs.get(&hyp.name) {
        hyp.goto_locations = l.clone();
    }
}
