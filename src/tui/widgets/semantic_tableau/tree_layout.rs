//! Tree layout calculation for the semantic tableau.

use crate::tui_ipc::{NodeId, ProofDag, ProofDagNode};

pub const MIN_BRANCH_WIDTH: u16 = 25;

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

/// Calculate height for a node box.
pub fn node_height(node: &ProofDagNode) -> u16 {
    3 + u16::from(!node.new_hypotheses.is_empty())
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

    if node.children.is_empty() {
        return (i32::from(MIN_BRANCH_WIDTH), h);
    }

    let (total_w, max_child_h) = node.children.iter().fold((0, 0), |(tw, mh), &cid| {
        let (cw, ch) = subtree_size(dag, cid);
        (tw + cw.max(i32::from(MIN_BRANCH_WIDTH)), mh.max(ch))
    });

    (total_w.max(i32::from(MIN_BRANCH_WIDTH)), h + max_child_h)
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

    let node_y = if top_down { y } else { y + available_h - box_h };

    out.push(NodePosition {
        node_id,
        x,
        y: node_y,
        width: u16::try_from(subtree_w).unwrap_or(MIN_BRANCH_WIDTH),
        height: node_height(node),
    });

    if !node.children.is_empty() {
        let child_h = available_h - box_h;
        let child_y = if top_down { y + box_h } else { y };
        let mut cx = x;

        for &cid in &node.children {
            let (cw, ch) = subtree_size(dag, cid);
            let cw = cw.max(i32::from(MIN_BRANCH_WIDTH));
            position_nodes(dag, cid, cx, child_y, child_h.min(ch), top_down, out);
            cx += cw;
        }
    }
}
