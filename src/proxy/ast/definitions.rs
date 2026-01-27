//! Definition (theorem, lemma, def, example) discovery.

use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Tree};

use crate::tui_ipc::Position;

/// Definition node kinds that can contain proofs.
pub const DEFINITION_KINDS: &[&str] = &["theorem", "lemma", "def", "example"];

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
