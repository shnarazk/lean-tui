//! Tree structure building algorithms for `ProofDag`.

use std::collections::HashMap;

use tracing::debug;

use super::{node::ProofDagNode, NodeId, ProofDag};
use crate::lean_rpc::PaperproofStep;

/// Index mapping goal IDs to the steps that work on them.
struct GoalStepIndex<'a> {
    steps: &'a [PaperproofStep],
    goal_to_steps: HashMap<String, Vec<usize>>,
}

impl<'a> GoalStepIndex<'a> {
    fn new(steps: &'a [PaperproofStep]) -> Self {
        let mut goal_to_steps: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, step) in steps.iter().enumerate() {
            let goal_id = &step.goal_before.id;
            goal_to_steps.entry(goal_id.clone()).or_default().push(i);
        }
        Self {
            steps,
            goal_to_steps,
        }
    }

    /// Check if a goal has a solver step after the given step index.
    fn goal_has_solver(&self, goal_id: &str, after_step: usize) -> bool {
        self.goal_to_steps
            .get(goal_id)
            .is_some_and(|indices| indices.iter().any(|&i| i > after_step))
    }

    /// Analyze goals and return child goal IDs and whether any goals are
    /// unsolved.
    fn analyze_step_goals(&self, step_idx: usize, node_id: NodeId) -> (Vec<String>, bool) {
        let step = &self.steps[step_idx];
        let mut child_goal_ids: Vec<String> = Vec::new();
        let mut has_unsolved = false;

        // Check spawned_goals (inline proofs like `by` blocks in `have`, `obtain`)
        for g in &step.spawned_goals {
            let solved = self.goal_has_solver(&g.id, step_idx);
            debug!(node_id, goal_id = %g.id, solved, "spawned_goal");
            if !solved {
                has_unsolved = true;
            }
            if !child_goal_ids.contains(&g.id) {
                child_goal_ids.push(g.id.clone());
            }
        }

        // Check goals_after (continuation goals after tactic completes)
        for g in &step.goals_after {
            let solved = self.goal_has_solver(&g.id, step_idx);
            debug!(node_id, goal_id = %g.id, solved, "goal_after");
            if !solved {
                has_unsolved = true;
            }
            if !child_goal_ids.contains(&g.id) {
                child_goal_ids.push(g.id.clone());
            }
        }

        (child_goal_ids, has_unsolved)
    }
}

/// Parameters for a branch being built in the tree.
#[derive(Clone, Copy)]
struct BranchParams<'a> {
    goal_id: &'a str,
    start_from: usize,
    parent_id: Option<NodeId>,
    depth: usize,
}

/// Build tree structure from Paperproof steps using goal IDs.
pub fn build_tree_structure(dag: &mut ProofDag, steps: &[PaperproofStep]) {
    if steps.is_empty() {
        return;
    }

    let goal_index = GoalStepIndex::new(steps);

    // Build tree recursively starting from root step's goal
    let root_idx = dag.root.unwrap_or(0) as usize;
    let root_goal_id = &steps[root_idx].goal_before.id;
    let root_params = BranchParams {
        goal_id: root_goal_id,
        start_from: 0,
        parent_id: None,
        depth: 0,
    };
    build_branch_recursive(dag, &goal_index, root_params);

    // Connect any orphan nodes not reached by goal-ID traversal
    connect_orphan_nodes(dag, steps);
}

/// Recursively build a branch of the tree.
fn build_branch_recursive(
    dag: &mut ProofDag,
    goal_index: &GoalStepIndex<'_>,
    params: BranchParams<'_>,
) -> Option<NodeId> {
    let step_indices = goal_index.goal_to_steps.get(params.goal_id)?;
    let &step_idx = step_indices.iter().find(|&&i| i >= params.start_from)?;

    let node_id = step_idx as NodeId;

    // Analyze goals first (before mutating dag)
    let (child_goal_ids, has_unsolved) = goal_index.analyze_step_goals(step_idx, node_id);

    // Update node with tree structure and unsolved status
    if let Some(node) = dag.nodes.get_mut(step_idx) {
        node.parent = params.parent_id;
        node.depth = params.depth;
        node.has_unsolved_spawned_goals = has_unsolved;
    }

    // Add this node as child of parent
    if let Some(pid) = params.parent_id {
        if let Some(parent_node) = dag.nodes.get_mut(pid as usize) {
            if !parent_node.children.contains(&node_id) {
                parent_node.children.push(node_id);
            }
        }
    }

    // Recursively process children
    for child_goal_id in child_goal_ids {
        let child_params = BranchParams {
            goal_id: &child_goal_id,
            start_from: step_idx + 1,
            parent_id: Some(node_id),
            depth: params.depth + 1,
        };
        build_branch_recursive(dag, goal_index, child_params);
    }

    Some(node_id)
}

/// Connect orphan nodes that weren't reached during goal-ID-based tree
/// building. Only connects orphans when there's a definite goal ID match - no
/// heuristics.
fn connect_orphan_nodes(dag: &mut ProofDag, steps: &[PaperproofStep]) {
    let root_id = dag.root.unwrap_or(0);

    // Find nodes with no parent (except root)
    let orphan_ids: Vec<NodeId> = dag
        .nodes
        .iter()
        .filter(|n| n.parent.is_none() && n.id != root_id)
        .map(|n| n.id)
        .collect();

    debug!(
        orphan_count = orphan_ids.len(),
        root_id,
        total_nodes = dag.nodes.len(),
        "Looking for orphan nodes"
    );

    for orphan_id in orphan_ids {
        let orphan_step = &steps[orphan_id as usize];
        let orphan_tactic = &orphan_step.tactic_string;

        debug!(
            orphan_id,
            orphan_tactic,
            orphan_goal_id = %orphan_step.goal_before.id,
            line = dag.nodes[orphan_id as usize].position.line,
            "Processing orphan node"
        );

        // Only connect if we find a definite goal ID match
        let parent_by_goal = dag.nodes.iter().find(|n| {
            let step = &steps[n.id as usize];
            step.goals_after
                .iter()
                .any(|g| g.id == orphan_step.goal_before.id)
                || step
                    .spawned_goals
                    .iter()
                    .any(|g| g.id == orphan_step.goal_before.id)
        });

        let Some(parent) = parent_by_goal else {
            debug!(
                orphan_id,
                orphan_tactic, "No goal ID match found - marking as detached orphan"
            );
            dag.orphans.push(orphan_id);
            continue;
        };

        let parent_id = parent.id;
        debug!(
            orphan_id,
            parent_id,
            parent_tactic = %steps[parent_id as usize].tactic_string,
            "Connecting orphan to parent via goal ID"
        );

        // Connect orphan to parent
        let parent_depth = dag.nodes[parent_id as usize].depth;
        if let Some(orphan_node) = dag.nodes.get_mut(orphan_id as usize) {
            orphan_node.parent = Some(parent_id);
            orphan_node.depth = parent_depth + 1;
        }
        if let Some(parent_node) = dag.nodes.get_mut(parent_id as usize) {
            if !parent_node.children.contains(&orphan_id) {
                parent_node.children.push(orphan_id);
            }
        }
    }
}

/// Build tree structure for local tactics based on depth.
pub fn build_local_tree_structure(nodes: &mut [ProofDagNode]) {
    if nodes.is_empty() {
        return;
    }

    // Use a stack to track the current parent at each depth level
    let mut depth_stack: Vec<NodeId> = vec![0];

    for i in 1..nodes.len() {
        let current_depth = nodes[i].depth;
        let prev_depth = nodes[i - 1].depth;

        if current_depth > prev_depth {
            // Going deeper: previous node is parent
            depth_stack.push((i - 1) as NodeId);
        } else if current_depth < prev_depth {
            // Coming back up: pop stack until we find right level
            while depth_stack.len() > current_depth + 1 {
                depth_stack.pop();
            }
        }

        // Set parent from stack
        if let Some(&parent_id) = depth_stack.last() {
            nodes[i].parent = Some(parent_id);

            // Add as child of parent
            let child_id = i as NodeId;
            nodes[parent_id as usize].children.push(child_id);
        }
    }
}
