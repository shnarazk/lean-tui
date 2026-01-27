//! Tactic enumeration within proofs.

use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Point, Tree};

use super::{case_splits::CASE_SPLITTING_TACTICS, definitions::DEFINITION_KINDS};
use crate::tui_ipc::Position;

/// Information about a tactic in the proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TacticInfo {
    /// The tactic text.
    pub text: String,
    /// Start position.
    pub start: Position,
    /// End position.
    pub end: Position,
    /// Nesting depth (for have/cases scopes).
    pub depth: usize,
}

/// Find all tactics within the enclosing definition.
///
/// Returns tactics in source order, which can be used to fetch goals
/// at each position and build a proof history.
#[allow(clippy::cast_possible_truncation)]
pub fn find_all_tactics_in_proof(
    tree: &Tree,
    source: &str,
    current: Position,
) -> Vec<TacticInfo> {
    let point = Point::new(current.line as usize, current.character as usize);

    // First find the enclosing definition
    let Some(def_node) = find_definition_node(tree.root_node(), point) else {
        return vec![];
    };

    // Find all tactics within this definition
    let mut tactics = Vec::new();
    collect_tactics_recursive(def_node, source, 0, &mut tactics);

    // Sort by position (should already be in order, but ensure it)
    tactics.sort_by(|a, b| {
        a.start
            .line
            .cmp(&b.start.line)
            .then(a.start.character.cmp(&b.start.character))
    });

    tactics
}

/// Find the definition node containing the given point.
fn find_definition_node(node: Node<'_>, point: Point) -> Option<Node<'_>> {
    // Check if this node contains our point
    if point < node.start_position() || point > node.end_position() {
        return None;
    }

    // If this is a definition node, return it
    if DEFINITION_KINDS.contains(&node.kind()) {
        return Some(node);
    }

    // Search children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_definition_node(child, point) {
            return Some(found);
        }
    }

    None
}

/// Recursively collect all tactics within a node.
#[allow(clippy::cast_possible_truncation)]
fn collect_tactics_recursive(
    node: Node<'_>,
    source: &str,
    depth: usize,
    tactics: &mut Vec<TacticInfo>,
) {
    // Check if this is a tactic node (direct child of "tactics" block)
    // or other tactic-like constructs
    let dominated = node.kind() == "apply"
        || node.kind() == "exact"
        || node.kind() == "have"
        || node.kind() == "let"
        || node.kind() == "show"
        || node.kind() == "suffices"
        || node.kind() == "calc"
        || node.kind() == "match_expr";

    if dominated {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            // Trim to first line for readability
            let first_line = text.lines().next().unwrap_or(text);
            let display_text = if first_line.len() > 50 {
                format!("{}...", &first_line[..47])
            } else {
                first_line.to_string()
            };

            tactics.push(TacticInfo {
                text: display_text,
                start: Position {
                    line: node.start_position().row as u32,
                    character: node.start_position().column as u32,
                },
                end: Position {
                    line: node.end_position().row as u32,
                    character: node.end_position().column as u32,
                },
                depth,
            });
        }
    }

    // Increase depth for scope-creating constructs
    let new_depth = if node.kind() == "have"
        || node.kind() == "let"
        || node.kind() == "match_expr"
        || CASE_SPLITTING_TACTICS
            .iter()
            .any(|t| node.utf8_text(source.as_bytes()).is_ok_and(|s| s.starts_with(t)))
    {
        depth + 1
    } else {
        depth
    };

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tactics_recursive(child, source, new_depth, tactics);
    }
}
