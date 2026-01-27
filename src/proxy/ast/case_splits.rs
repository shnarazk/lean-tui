//! Case-splitting tactic detection.

use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Point, Tree};

use super::util::find_smallest_containing_node;
use crate::tui_ipc::Position;

/// Case-splitting tactic names that create multiple goals.
pub const CASE_SPLITTING_TACTICS: &[&str] = &[
    "by_cases",
    "cases",
    "rcases",
    "obtain",
    "induction",
    "match",
    "split",
    "constructor",
];

/// Information about a case-splitting tactic that creates the current proof
/// branches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseSplitInfo {
    /// Name of the tactic (e.g., `by_cases`, `cases`)
    pub tactic: String,
    /// The hypothesis or discriminant name (e.g., `hprime` in `by_cases
    /// hprime`)
    pub name: Option<String>,
    /// Line where the tactic starts
    pub line: u32,
}

/// Find case-splitting tactics that affect the given position.
/// Looks for case-splits that start before the current line within the same
/// block.
pub fn find_case_splits(tree: &Tree, source: &str, current: Position) -> Vec<CaseSplitInfo> {
    let point = Point::new(current.line as usize, current.character as usize);
    let mut results = Vec::new();

    // Find the smallest node containing our point
    let containing = find_smallest_containing_node(tree.root_node(), point);
    if let Some(node) = containing {
        // Walk up to find case-splits in ancestor nodes
        collect_case_splits_in_ancestors(node, source, point, &mut results);
    }

    // Deduplicate (same tactic might be found via multiple paths)
    results.sort_by_key(|info| info.line);
    results.dedup();
    results
}

/// Collect case-splits from ancestors that start before the current point.
fn collect_case_splits_in_ancestors(
    node: Node<'_>,
    source: &str,
    point: Point,
    results: &mut Vec<CaseSplitInfo>,
) {
    // Walk through siblings that come before our position
    if let Some(parent) = node.parent() {
        let mut cursor = parent.walk();
        for sibling in parent.children(&mut cursor) {
            // Only look at siblings that start before our point
            if sibling.start_position() >= point {
                continue;
            }

            // Check this sibling and its descendants for case-splits
            collect_case_splits_in_node(sibling, source, results);
        }

        // Continue walking up the tree
        collect_case_splits_in_ancestors(parent, source, point, results);
    }
}

/// Collect all case-splits within a node (recursively).
fn collect_case_splits_in_node(node: Node<'_>, source: &str, results: &mut Vec<CaseSplitInfo>) {
    if node.kind() == "apply" {
        if let Some(info) = extract_case_split_info(node, source) {
            results.push(info);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_case_splits_in_node(child, source, results);
    }
}

/// Extract case split info from an apply node if it's a case-splitting tactic.
#[allow(clippy::cast_possible_truncation)]
fn extract_case_split_info(node: Node<'_>, source: &str) -> Option<CaseSplitInfo> {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    // First child should be the tactic name (identifier)
    let first = children.first()?;
    if first.kind() != "identifier" {
        return None;
    }

    let tactic_name = first.utf8_text(source.as_bytes()).ok()?;
    if !CASE_SPLITTING_TACTICS.contains(&tactic_name) {
        return None;
    }

    // Second child might be the hypothesis/discriminant name
    let name = children.get(1).and_then(|n| {
        if n.kind() == "identifier" {
            n.utf8_text(source.as_bytes()).ok().map(String::from)
        } else {
            None
        }
    });

    Some(CaseSplitInfo {
        tactic: tactic_name.to_string(),
        name,
        line: node.start_position().row as u32,
    })
}
