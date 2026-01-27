//! Proof navigation (previous/next tactic).

use tree_sitter::{Node, Point, Tree};

use crate::tui_ipc::Position;

/// Find the position of the previous tactic before the current position.
pub fn find_previous_tactic(tree: &Tree, current: Position) -> Option<Position> {
    let point = Point::new(current.line as usize, current.character as usize);
    let tactics_block = find_enclosing_tactics(tree.root_node(), point)?;

    // Find the last tactic that starts before our position
    let mut prev_tactic: Option<Node> = None;
    let mut cursor = tactics_block.walk();
    for child in tactics_block.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.start_position() < point {
            prev_tactic = Some(child);
        } else {
            break;
        }
    }

    prev_tactic.map(node_to_position)
}

/// Find the position of the next tactic after the current position.
pub fn find_next_tactic(tree: &Tree, current: Position) -> Option<Position> {
    let point = Point::new(current.line as usize, current.character as usize);
    let tactics_block = find_enclosing_tactics(tree.root_node(), point)?;

    // Find the first tactic that starts after our position
    let mut cursor = tactics_block.walk();
    for child in tactics_block.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        // Must start strictly after current position (not at same position)
        if child.start_position() > point {
            return Some(node_to_position(child));
        }
    }

    None
}

/// Find the "tactics" node that contains the given position.
fn find_enclosing_tactics(root: Node<'_>, point: Point) -> Option<Node<'_>> {
    find_tactics_recursive(root, point)
}

fn find_tactics_recursive(node: Node<'_>, point: Point) -> Option<Node<'_>> {
    // Check if this node contains our point
    if point < node.start_position() || point > node.end_position() {
        return None;
    }

    // If this is a tactics node, return it
    if node.kind() == "tactics" {
        return Some(node);
    }

    // Search children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_tactics_recursive(child, point) {
            return Some(found);
        }
    }

    None
}

/// Convert a tree-sitter node's start position to our Position type.
#[allow(clippy::cast_possible_truncation)] // Source positions won't exceed u32
fn node_to_position(node: Node<'_>) -> Position {
    let start = node.start_position();
    Position {
        line: start.row as u32,
        character: start.column as u32,
    }
}
