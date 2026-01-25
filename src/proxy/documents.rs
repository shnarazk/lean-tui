//! Document content cache with tree-sitter parsing.
//!
//! Caches document content and syntax trees for efficient tactic position detection.
//! Tree-sitter's incremental parsing reuses unchanged subtrees automatically.

use std::{collections::HashMap, sync::Mutex};

use async_lsp::{
    lsp_types::{
        notification::{DidChangeTextDocument, DidOpenTextDocument, Notification},
        DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    },
    AnyNotification,
};
use tree_sitter::{Parser, Tree};

/// Cached document with content and syntax tree.
struct Document {
    content: String,
    tree: Option<Tree>,
}

/// Document cache with tree-sitter parsing.
/// Uses `std::sync::Mutex` because tree-sitter's `Tree` is not `Send`.
pub struct DocumentCache {
    documents: Mutex<HashMap<String, Document>>,
    parser: Mutex<Parser>,
}

impl DocumentCache {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(tree_sitter_lean::language())
            .expect("Error loading Lean grammar");

        Self {
            documents: Mutex::new(HashMap::new()),
            parser: Mutex::new(parser),
        }
    }

    /// Process LSP notification, updating cache for document events.
    pub fn handle_notification(&self, notif: &AnyNotification) {
        if notif.method == DidOpenTextDocument::METHOD {
            let Ok(p) = serde_json::from_value::<DidOpenTextDocumentParams>(notif.params.clone())
            else {
                return;
            };
            self.update(p.text_document.uri.as_str(), p.text_document.text);
        } else if notif.method == DidChangeTextDocument::METHOD {
            let Ok(p) = serde_json::from_value::<DidChangeTextDocumentParams>(notif.params.clone())
            else {
                return;
            };
            let uri = p.text_document.uri.to_string();
            if let Some(content) = self.apply_changes(&uri, &p.content_changes) {
                self.update(&uri, content);
            }
        }
    }

    /// Update document content and re-parse with old tree for incremental benefit.
    fn update(&self, uri: &str, content: String) {
        let mut docs = self.documents.lock().expect("lock poisoned");
        let old_tree = docs.get(uri).and_then(|d| d.tree.as_ref());
        let tree = self.parse(&content, old_tree);
        docs.insert(uri.to_string(), Document { content, tree });
    }

    /// Apply LSP content changes and return the new content.
    fn apply_changes(
        &self,
        uri: &str,
        changes: &[async_lsp::lsp_types::TextDocumentContentChangeEvent],
    ) -> Option<String> {
        let docs = self.documents.lock().expect("lock poisoned");
        let mut content = docs.get(uri)?.content.clone();
        drop(docs);

        for change in changes {
            let Some(range) = change.range else {
                content.clone_from(&change.text);
                continue;
            };
            let start = position_to_offset(&content, range.start);
            let end = position_to_offset(&content, range.end);
            if start <= content.len() && end <= content.len() {
                content.replace_range(start..end, &change.text);
            }
        }
        Some(content)
    }

    fn parse(&self, content: &str, old_tree: Option<&Tree>) -> Option<Tree> {
        self.parser.lock().expect("lock poisoned").parse(content, old_tree)
    }

    /// Get cached syntax tree for a document.
    pub fn get_tree(&self, uri: &str) -> Option<Tree> {
        self.documents.lock().expect("lock poisoned")
            .get(uri)
            .and_then(|d| d.tree.clone())
    }
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert LSP position to byte offset.
fn position_to_offset(content: &str, pos: async_lsp::lsp_types::Position) -> usize {
    content
        .lines()
        .take(pos.line as usize)
        .map(|line| line.len() + 1)
        .sum::<usize>()
        + pos.character as usize
}
