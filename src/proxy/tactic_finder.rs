//! Tactic position finder using tree-sitter.
//!
//! Uses the tree-sitter-lean grammar to find previous/next tactic positions.
//! A tactic is identified as a direct child of a "tactics" node.

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

#[cfg(test)]
mod tests {
    use tree_sitter::Parser;

    use super::*;

    fn parse(code: &str) -> Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_lean4_source::language())
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
        println!("From intro (line 1), next tactic: {:?}", next);

        // This test documents the current behavior - we expect it to fail
        // because the grammar doesn't create a proper tactics block
        if next.is_none() {
            println!("WARNING: Grammar does not support navigation between have tactics!");
            println!("The tree structure shows have statements are not inside a 'tactics' node.");
        }
    }
}
