//! Shared tree-sitter utilities.

use tree_sitter::{Node, Point};

/// Find the smallest node that contains the given point.
pub fn find_smallest_containing_node(node: Node<'_>, point: Point) -> Option<Node<'_>> {
    if point < node.start_position() || point > node.end_position() {
        return None;
    }

    // Try to find a smaller containing child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_smallest_containing_node(child, point) {
            return Some(found);
        }
    }

    Some(node)
}
