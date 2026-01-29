//! Tactic enumeration within proofs.

use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Point, Tree};

use super::definitions::DEFINITION_KINDS;
use crate::tui_ipc::Position;

/// Tactics that create branching/case structure in proofs.
const CASE_SPLITTING_TACTICS: &[&str] = &["cases", "induction", "rcases", "obtain", "by_cases"];

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

/// Context for tactic collection traversal.
struct TacticCollector<'a> {
    source: &'a str,
    tactics: Vec<TacticInfo>,
}

/// Truncate a string to display length, adding ellipsis if needed.
fn truncate_display(text: &str, max_len: usize) -> String {
    if text.len() > max_len {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    } else {
        text.to_string()
    }
}

impl<'a> TacticCollector<'a> {
    const fn new(source: &'a str) -> Self {
        Self {
            source,
            tactics: Vec::new(),
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn collect_recursive(&mut self, node: Node<'_>, depth: usize) {
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
            if let Ok(text) = node.utf8_text(self.source.as_bytes()) {
                let first_line = text.lines().next().unwrap_or(text);
                self.tactics.push(TacticInfo {
                    text: truncate_display(first_line, 50),
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
            || CASE_SPLITTING_TACTICS.iter().any(|t| {
                node.utf8_text(self.source.as_bytes())
                    .is_ok_and(|s| s.starts_with(t))
            }) {
            depth + 1
        } else {
            depth
        };

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_recursive(child, new_depth);
        }
    }
}

/// Find all tactics within the enclosing definition.
///
/// Returns tactics in source order, which can be used to fetch goals
/// at each position and build a proof history.
#[allow(clippy::cast_possible_truncation)]
pub fn find_all_tactics_in_proof(tree: &Tree, source: &str, current: Position) -> Vec<TacticInfo> {
    let point = Point::new(current.line as usize, current.character as usize);

    // First find the enclosing definition
    let Some(def_node) = find_definition_node(tree.root_node(), point) else {
        return vec![];
    };

    // Find all tactics within this definition
    let mut collector = TacticCollector::new(source);
    collector.collect_recursive(def_node, 0);

    // Sort by position (should already be in order, but ensure it)
    collector.tactics.sort_by(|a, b| {
        a.start
            .line
            .cmp(&b.start.line)
            .then(a.start.character.cmp(&b.start.character))
    });

    collector.tactics
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
