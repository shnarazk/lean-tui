//! Tests for AST analysis functions.

use tree_sitter::{Parser, Tree};

use super::*;
use crate::tui_ipc::Position;

fn parse(code: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_lean4::language())
        .expect("Error loading Lean grammar");
    parser.parse(code, None).expect("Failed to parse")
}

#[test]
fn test_find_previous_tactic() {
    let code = "theorem foo : True := by\n  trivial\n\n  done";
    let tree = parse(code);
    let pos = Position {
        line: 3,
        character: 2,
    };
    let prev = find_previous_tactic(&tree, pos);
    assert!(prev.is_some());
    assert_eq!(prev.unwrap().line, 1);
}

#[test]
fn test_find_next_tactic() {
    let code = "theorem foo : True := by\n  trivial\n\n  done";
    let tree = parse(code);
    let pos = Position {
        line: 1,
        character: 2,
    };
    let next = find_next_tactic(&tree, pos);
    assert!(next.is_some());
    assert_eq!(next.unwrap().line, 3);
}
