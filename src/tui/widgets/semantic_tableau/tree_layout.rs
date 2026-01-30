//! Tree layout calculation for the semantic tableau.

use crate::lean_rpc::dag::{NodeId, ProofDag, ProofDagNode};

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

/// Configuration for tree layout direction and data source.
struct TreeLayoutConfig<'a> {
    dag: &'a ProofDag,
    top_down: bool,
}

/// Placement target for a node in the layout grid.
#[derive(Clone, Copy)]
struct NodePlacement {
    node_id: NodeId,
    x: i32,
    y: i32,
    available_h: i32,
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
            let hyp_width = h.name.len() + h.type_.to_plain_text().len() + 5;
            max_width = max_width.max(hyp_width);
        }
    }

    // Goal widths: "⊢ type" or "✓ Goal completed"
    if node.state_after.goals.is_empty() {
        max_width = max_width.max(16); // "✓ Goal completed"
    } else {
        for g in &node.state_after.goals {
            let goal_width = g.type_.to_plain_text().len() + 4; // "⊢ " prefix + padding
            max_width = max_width.max(goal_width);
        }
    }

    // Clamp to min/max and add border padding
    let width = (max_width + 4) as u16;
    width.clamp(MIN_NODE_WIDTH, MAX_NODE_WIDTH)
}

/// Gap between main tree and orphan nodes.
const ORPHAN_GAP: i32 = 4;

/// Calculate tree layout with actual content dimensions.
pub fn calculate_tree_layout(dag: &ProofDag, top_down: bool) -> TreeLayout {
    let mut layout = TreeLayout::default();

    let Some(root_id) = dag.root else {
        return layout;
    };

    let (w, h) = subtree_size(dag, root_id);
    layout.content_width = w;
    layout.content_height = h;

    let config = TreeLayoutConfig { dag, top_down };
    let root_placement = NodePlacement {
        node_id: root_id,
        x: 0,
        y: 0,
        available_h: h,
    };
    position_nodes(&config, root_placement, &mut layout.nodes);

    // Position orphan nodes to the right of the main tree
    if !dag.orphans.is_empty() {
        let orphan_x = layout.content_width + ORPHAN_GAP;
        let mut orphan_y = 0i32;

        for &orphan_id in &dag.orphans {
            if let Some(node) = dag.get(orphan_id) {
                let node_w = node_content_width(node);
                let node_h = node_height(node);

                layout.nodes.push(NodePosition {
                    node_id: orphan_id,
                    x: orphan_x,
                    y: orphan_y,
                    width: node_w,
                    height: node_h,
                });

                orphan_y += i32::from(node_h) + 1;
                layout.content_width = layout.content_width.max(orphan_x + i32::from(node_w));
                layout.content_height = layout.content_height.max(orphan_y);
            }
        }
    }

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
    config: &TreeLayoutConfig<'_>,
    placement: NodePlacement,
    out: &mut Vec<NodePosition>,
) {
    let Some(node) = config.dag.get(placement.node_id) else {
        return;
    };

    let box_h = i32::from(node_height(node));
    let (subtree_w, _) = subtree_size(config.dag, placement.node_id);
    let node_w = node_content_width(node);

    let node_y = if config.top_down {
        placement.y
    } else {
        placement.y + placement.available_h - box_h
    };

    out.push(NodePosition {
        node_id: placement.node_id,
        x: placement.x,
        y: node_y,
        width: u16::try_from(subtree_w).unwrap_or(node_w).max(node_w),
        height: node_height(node),
    });

    if !node.children.is_empty() {
        let child_h = placement.available_h - box_h;
        let child_y = if config.top_down {
            placement.y + box_h
        } else {
            placement.y
        };
        let mut cx = placement.x;

        for &cid in &node.children {
            let (cw, ch) = subtree_size(config.dag, cid);
            let child_placement = NodePlacement {
                node_id: cid,
                x: cx,
                y: child_y,
                available_h: child_h.min(ch),
            };
            position_nodes(config, child_placement, out);
            cx += cw;
        }
    }
}
