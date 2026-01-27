//! Tree structure building algorithms for `ProofDag`.

use std::collections::HashMap;

use super::{node::ProofDagNode, NodeId, ProofDag};
use crate::lean_rpc::PaperproofStep;

/// Build tree structure from Paperproof steps using goal IDs.
pub fn build_tree_structure(dag: &mut ProofDag, steps: &[PaperproofStep]) {
    if steps.is_empty() {
        return;
    }

    // Build a map from goal_id to the steps that work on that goal
    let mut goal_to_steps: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, step) in steps.iter().enumerate() {
        let goal_id = &step.goal_before.id;
        goal_to_steps.entry(goal_id.clone()).or_default().push(i);
    }

    // Build tree recursively starting from first step's goal
    let root_goal_id = &steps[0].goal_before.id;
    build_branch_recursive(dag, steps, &goal_to_steps, root_goal_id, 0, None, 0, 1, 0);

    // Update is_leaf flags
    for node in &mut dag.nodes {
        node.is_leaf = node.children.is_empty();
    }
}

/// Recursively build a branch of the tree.
#[allow(clippy::too_many_arguments)]
fn build_branch_recursive(
    dag: &mut ProofDag,
    steps: &[PaperproofStep],
    goal_to_steps: &HashMap<String, Vec<usize>>,
    goal_id: &str,
    start_from: usize,
    parent_id: Option<NodeId>,
    sibling_index: usize,
    sibling_count: usize,
    depth: usize,
) -> Option<NodeId> {
    let step_indices = goal_to_steps.get(goal_id)?;
    let &step_idx = step_indices.iter().find(|&&i| i >= start_from)?;

    let node_id = step_idx as NodeId;
    let step = &steps[step_idx];

    // Update node with tree structure info
    if let Some(node) = dag.nodes.get_mut(step_idx) {
        node.parent = parent_id;
        node.sibling_index = sibling_index;
        node.sibling_count = sibling_count;
        node.depth = depth;
    }

    // Add this node as child of parent
    if let Some(pid) = parent_id {
        if let Some(parent_node) = dag.nodes.get_mut(pid as usize) {
            if !parent_node.children.contains(&node_id) {
                parent_node.children.push(node_id);
            }
        }
    }

    // Determine children based on spawned goals or goals_after
    let child_goal_ids: Vec<String> = if !step.spawned_goals.is_empty() {
        step.spawned_goals.iter().map(|g| g.id.clone()).collect()
    } else if step.goals_after.len() > 1 {
        step.goals_after.iter().map(|g| g.id.clone()).collect()
    } else if let Some(next_goal) = step.goals_after.first() {
        vec![next_goal.id.clone()]
    } else {
        vec![]
    };

    let child_count = child_goal_ids.len();
    for (i, child_goal_id) in child_goal_ids.into_iter().enumerate() {
        build_branch_recursive(
            dag,
            steps,
            goal_to_steps,
            &child_goal_id,
            step_idx + 1,
            Some(node_id),
            i,
            child_count,
            depth + 1,
        );
    }

    Some(node_id)
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

    // Update sibling info
    for i in 0..nodes.len() {
        let children = nodes[i].children.clone();
        let count = children.len();
        for (idx, &child_id) in children.iter().enumerate() {
            if let Some(child) = nodes.get_mut(child_id as usize) {
                child.sibling_index = idx;
                child.sibling_count = count;
            }
        }
    }

    // Update is_leaf flags
    for node in nodes.iter_mut() {
        node.is_leaf = node.children.is_empty();
    }
}
