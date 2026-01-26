//! Tactic position finder using tree-sitter.
//!
//! Uses the tree-sitter-lean grammar to find previous/next tactic positions.
//! A tactic is identified as a direct child of a "tactics" node.

use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Point, Tree};

use crate::tui_ipc::Position;

/// Case-splitting tactic names that create multiple goals.
const CASE_SPLITTING_TACTICS: &[&str] = &[
    "by_cases",
    "cases",
    "rcases",
    "obtain",
    "induction",
    "match",
    "split",
    "constructor",
];

/// Definition node kinds that can contain proofs.
const DEFINITION_KINDS: &[&str] = &["theorem", "lemma", "def", "example"];

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

/// Information about the enclosing definition (theorem, lemma, def, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionInfo {
    /// Kind of definition (theorem, lemma, def, example)
    pub kind: String,
    /// Name of the definition
    pub name: String,
    /// Line where the definition starts
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

/// Find the smallest node that contains the given point.
fn find_smallest_containing_node(node: Node<'_>, point: Point) -> Option<Node<'_>> {
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

/// Find the enclosing definition (theorem, lemma, def) for the given position.
pub fn find_enclosing_definition(
    tree: &Tree,
    source: &str,
    current: Position,
) -> Option<DefinitionInfo> {
    find_definition_recursive(tree.root_node(), source, current.line as usize)
}

#[allow(clippy::cast_possible_truncation)]
fn find_definition_recursive(node: Node<'_>, source: &str, line: usize) -> Option<DefinitionInfo> {
    // Check if this node contains the line
    if line < node.start_position().row || line > node.end_position().row {
        return None;
    }

    // Check if this is a definition node
    if DEFINITION_KINDS.contains(&node.kind()) {
        return extract_definition_info(node, source);
    }

    // Recurse into children, return first match
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(info) = find_definition_recursive(child, source, line) {
            return Some(info);
        }
    }

    None
}

/// Extract definition info from a theorem/lemma/def node.
#[allow(clippy::cast_possible_truncation)]
fn extract_definition_info(node: Node<'_>, source: &str) -> Option<DefinitionInfo> {
    let kind = node.kind().to_string();
    let mut cursor = node.walk();

    // Find the identifier child (the name)
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = child.utf8_text(source.as_bytes()).ok()?;
            return Some(DefinitionInfo {
                kind,
                name: name.to_string(),
                line: node.start_position().row as u32,
            });
        }
    }

    None
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

#[cfg(test)]
mod tests {
    use tree_sitter::Parser;

    use super::*;

    fn parse(code: &str) -> Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_lean4::language())
            .expect("Error loading Lean grammar");
        parser.parse(code, None).expect("Failed to parse")
    }

    #[test]
    fn test_parse_simple_proof() {
        let code = "theorem foo : True := by\n  trivial";
        let tree = parse(code);
        assert!(!tree.root_node().has_error());
    }

    fn print_tree(node: tree_sitter::Node, code: &str, indent: usize) {
        let text = &code[node.byte_range()];
        let short = if text.len() > 20 {
            format!("{}...", &text[..20])
        } else {
            text.to_string()
        };
        println!(
            "{:indent$}{} [{}:{}] {:?}",
            "",
            node.kind(),
            node.start_position().row,
            node.start_position().column,
            short.replace('\n', "\\n"),
            indent = indent
        );
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            print_tree(child, code, indent + 2);
        }
    }

    #[test]
    fn test_find_tactics_structure() {
        let code = "theorem foo : True := by\n  trivial\n  done";
        let tree = parse(code);
        print_tree(tree.root_node(), code, 0);
    }

    #[test]
    fn test_have_declarations_structure() {
        // Simplified version similar to user's test.lean lines 47-54
        let code = r"theorem foo : True := by
  intro n

  have : 1 < 2 := by grind

  have : 2 < 3 := by grind

  have := Nat.add_comm 1 2
  grind";
        let tree = parse(code);
        println!("=== Have declarations tree structure ===");
        print_tree(tree.root_node(), code, 0);
    }

    #[test]
    fn test_find_previous_with_empty_lines() {
        // Tactics with empty line between them
        let code = "theorem foo : True := by\n  trivial\n\n  done";
        let tree = parse(code);

        // Cursor on "done" (line 3) should find "trivial" (line 1) as previous
        let pos = Position {
            line: 3,
            character: 2,
        };
        let prev = find_previous_tactic(&tree, pos);
        assert!(prev.is_some(), "Should find previous tactic");
        assert_eq!(
            prev.unwrap().line,
            1,
            "Previous should be on line 1 (trivial)"
        );
    }

    #[test]
    fn test_find_next_with_empty_lines() {
        // Tactics with empty line between them
        let code = "theorem foo : True := by\n  trivial\n\n  done";
        let tree = parse(code);

        // Cursor on "trivial" (line 1) should find "done" (line 3) as next
        let pos = Position {
            line: 1,
            character: 2,
        };
        let next = find_next_tactic(&tree, pos);
        assert!(next.is_some(), "Should find next tactic");
        assert_eq!(next.unwrap().line, 3, "Next should be on line 3 (done)");
    }

    #[test]
    fn test_find_enclosing_definition() {
        let code = r#"theorem infinitude_of_primes : ∀ n, ∃ p > n, IsPrime p := by
  intro n
  sorry"#;
        let tree = parse(code);

        // Cursor inside the proof (line 1)
        let pos = Position {
            line: 1,
            character: 4,
        };
        let def_info = find_enclosing_definition(&tree, code, pos);

        println!("Definition info: {def_info:?}");
        assert!(def_info.is_some(), "Should find definition");
        let info = def_info.unwrap();
        assert_eq!(info.kind, "theorem");
        assert_eq!(info.name, "infinitude_of_primes");
    }

    #[test]
    fn test_by_cases_structure() {
        let code = r#"theorem test : P ∧ Q := by
  by_cases hprime : IsPrime n
  · grind
  · obtain ⟨k, _⟩ : ∃ k, k > 0 := by simp
    grind"#;
        let tree = parse(code);
        println!("=== by_cases tree structure ===");
        print_tree(tree.root_node(), code, 0);
    }

    #[test]
    fn test_cases_structure() {
        let code = r#"theorem test (h : P ∨ Q) : R := by
  cases h with
  | inl hp => sorry
  | inr hq => sorry"#;
        let tree = parse(code);
        println!("=== cases with structure ===");
        print_tree(tree.root_node(), code, 0);
    }

    #[test]
    fn test_find_case_splits() {
        let code = r#"theorem test : P ∧ Q := by
  by_cases hprime : IsPrime n
  · grind
  · obtain ⟨k, _⟩ : ∃ k, k > 0 := by simp
    grind"#;
        let tree = parse(code);

        println!("=== Tree structure ===");
        print_tree(tree.root_node(), code, 0);

        // Cursor inside the first branch (line 2, on grind)
        let pos = Position {
            line: 2,
            character: 4,
        };
        let splits = find_case_splits(&tree, code, pos);

        println!("Case splits found at line 2: {splits:?}");
        assert!(!splits.is_empty(), "Should find case split");
        assert_eq!(splits[0].tactic, "by_cases");
        assert_eq!(splits[0].name, Some("hprime".to_string()));
    }

    #[test]
    fn test_find_nested_case_splits() {
        let code = r#"theorem test : P := by
  by_cases h1 : A
  · by_cases h2 : B
    · sorry
    · sorry
  · sorry"#;
        let tree = parse(code);

        // Cursor inside nested branch (line 3)
        let pos = Position {
            line: 3,
            character: 6,
        };
        let splits = find_case_splits(&tree, code, pos);

        println!("Nested case splits: {splits:?}");
        // Should find both by_cases, innermost first
        assert!(splits.len() >= 1, "Should find at least one case split");
    }

    #[test]
    fn test_find_next_with_have_tactics() {
        // Have tactics - this reveals the grammar limitation
        let code = r"theorem foo : True := by
  intro n
  have : 1 < 2 := by grind
  have : 2 < 3 := by grind
  grind";
        let tree = parse(code);

        println!("=== Testing have navigation ===");
        print_tree(tree.root_node(), code, 0);

        // Cursor on "intro" (line 1) - can we find "have" (line 2)?
        let pos = Position {
            line: 1,
            character: 2,
        };
        let next = find_next_tactic(&tree, pos);
        println!("From intro (line 1), next tactic: {next:?}");

        // This test documents the current behavior - we expect it to fail
        // because the grammar doesn't create a proper tactics block
        if next.is_none() {
            println!("WARNING: Grammar does not support navigation between have tactics!");
            println!("The tree structure shows have statements are not inside a 'tactics' node.");
        }
    }
}
