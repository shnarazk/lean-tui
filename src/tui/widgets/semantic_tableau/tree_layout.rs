//! Tree layout calculation for the semantic tableau.

use crate::tui_ipc::{NodeId, ProofDag, ProofDagNode};

pub const MIN_NODE_WIDTH: u16 = 25;
pub const MAX_NODE_WIDTH: u16 = 60;

#[derive(Debug, Clone, Copy)]
pub struct NodePosition {
    pub node_id: NodeId,
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Default)]
pub struct TreeLayout {
    pub nodes: Vec<NodePosition>,
    pub content_width: i32,
    pub content_height: i32,
}

impl TreeLayout {
    pub fn find_node(&self, node_id: NodeId) -> Option<&NodePosition> {
        self.nodes.iter().find(|n| n.node_id == node_id)
    }
}

/// Calculate height for a node box based on content.
pub const fn node_height(node: &ProofDagNode) -> u16 {
    // Border (2) + goals line (1) + optional hypotheses line (1)
    3 + (!node.new_hypotheses.is_empty()) as u16
}

/// Calculate minimum width needed for a node's content.
pub fn node_content_width(node: &ProofDagNode) -> u16 {
    let mut max_width: usize = 0;

    // Tactic title width (with " tactic [N→] " format)
    let title_width = node.tactic.text.len() + 8;
    max_width = max_width.max(title_width);

    // Hypothesis widths: " name: type "
    for &hyp_idx in &node.new_hypotheses {
        if let Some(h) = node.state_after.hypotheses.get(hyp_idx) {
            let hyp_width = h.name.len() + h.type_.len() + 5;
            max_width = max_width.max(hyp_width);
        }
    }

    // Goal widths: "⊢ type" or "✓ Goal completed"
    if node.state_after.goals.is_empty() {
        max_width = max_width.max(16); // "✓ Goal completed"
    } else {
        for g in &node.state_after.goals {
            let goal_width = g.type_.len() + 4; // "⊢ " prefix + padding
            max_width = max_width.max(goal_width);
        }
    }

    // Clamp to min/max and add border padding
    let width = (max_width + 4) as u16;
    width.clamp(MIN_NODE_WIDTH, MAX_NODE_WIDTH)
}

/// Calculate tree layout with actual content dimensions.
pub fn calculate_tree_layout(dag: &ProofDag, top_down: bool) -> TreeLayout {
    let mut layout = TreeLayout::default();

    let Some(root_id) = dag.root else {
        return layout;
    };

    let (w, h) = subtree_size(dag, root_id);
    layout.content_width = w;
    layout.content_height = h;

    position_nodes(dag, root_id, 0, 0, h, top_down, &mut layout.nodes);
    layout
}

/// Calculate subtree dimensions (width, height).
fn subtree_size(dag: &ProofDag, node_id: NodeId) -> (i32, i32) {
    let Some(node) = dag.get(node_id) else {
        return (0, 0);
    };

    let h = i32::from(node_height(node));
    let node_w = i32::from(node_content_width(node));

    if node.children.is_empty() {
        return (node_w, h);
    }

    let (total_w, max_child_h) = node.children.iter().fold((0, 0), |(tw, mh), &cid| {
        let (cw, ch) = subtree_size(dag, cid);
        (tw + cw, mh.max(ch))
    });

    (total_w.max(node_w), h + max_child_h)
}

/// Position nodes recursively.
fn position_nodes(
    dag: &ProofDag,
    node_id: NodeId,
    x: i32,
    y: i32,
    available_h: i32,
    top_down: bool,
    out: &mut Vec<NodePosition>,
) {
    let Some(node) = dag.get(node_id) else {
        return;
    };

    let box_h = i32::from(node_height(node));
    let (subtree_w, _) = subtree_size(dag, node_id);
    let node_w = node_content_width(node);

    let node_y = if top_down { y } else { y + available_h - box_h };

    out.push(NodePosition {
        node_id,
        x,
        y: node_y,
        width: u16::try_from(subtree_w).unwrap_or(node_w).max(node_w),
        height: node_height(node),
    });

    if !node.children.is_empty() {
        let child_h = available_h - box_h;
        let child_y = if top_down { y + box_h } else { y };
        let mut cx = x;

        for &cid in &node.children {
            let (cw, ch) = subtree_size(dag, cid);
            position_nodes(dag, cid, cx, child_y, child_h.min(ch), top_down, out);
            cx += cw;
        }
    }
}
