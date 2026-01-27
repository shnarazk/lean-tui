//! Tree builder - constructs a proof tree from Paperproof steps.

use std::collections::HashMap;

use crate::lean_rpc::PaperproofStep;

/// A node in the proof tree.
#[derive(Debug, Clone)]
#[allow(clippy::use_self)] // Self in Vec<Self> doesn't work well with derived traits
pub struct ProofNode {
    /// Index into the original steps array.
    pub step_index: usize,
    /// Children nodes (branches from case splits, etc.).
    pub children: Vec<ProofNode>,
}

/// Build a tree structure from Paperproof steps.
///
/// The tree represents the branching structure of the proof:
/// - Linear sequences of tactics form a single branch
/// - Case splits (`by_cases`, `cases`, `induction`) create multiple children
pub fn build_proof_tree(steps: &[PaperproofStep]) -> Option<ProofNode> {
    if steps.is_empty() {
        return None;
    }

    // Build a map from goal_id to the steps that work on that goal
    let mut goal_to_steps: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, step) in steps.iter().enumerate() {
        let goal_id = &step.goal_before.id;
        goal_to_steps.entry(goal_id.clone()).or_default().push(i);
    }

    // Start from the first step's goal
    let root_goal_id = &steps[0].goal_before.id;
    build_branch(steps, &goal_to_steps, root_goal_id, 0)
}

/// Build a branch of the tree starting from a specific goal.
fn build_branch(
    steps: &[PaperproofStep],
    goal_to_steps: &HashMap<String, Vec<usize>>,
    goal_id: &str,
    start_from: usize,
) -> Option<ProofNode> {
    let step_indices = goal_to_steps.get(goal_id)?;

    // Find the first step for this goal at or after start_from
    let &step_idx = step_indices.iter().find(|&&i| i >= start_from)?;
    let step = &steps[step_idx];

    // Check if this step spawns new goals (branches)
    let children = if !step.spawned_goals.is_empty() {
        // Create a child branch for each spawned goal
        step.spawned_goals
            .iter()
            .filter_map(|spawned| build_branch(steps, goal_to_steps, &spawned.id, step_idx + 1))
            .collect()
    } else if step.goals_after.len() > 1 {
        // Multiple goals after = branching (e.g., constructor, refine)
        step.goals_after
            .iter()
            .filter_map(|goal| build_branch(steps, goal_to_steps, &goal.id, step_idx + 1))
            .collect()
    } else if let Some(next_goal) = step.goals_after.first() {
        // Single goal after = continue the branch
        build_branch(steps, goal_to_steps, &next_goal.id, step_idx + 1)
            .map_or_else(Vec::new, |child| vec![child])
    } else {
        // No goals after = branch ends (goal solved)
        vec![]
    };

    Some(ProofNode {
        step_index: step_idx,
        children,
    })
}
