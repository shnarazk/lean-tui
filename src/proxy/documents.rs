//! Document content cache for tactic position detection.
//!
//! Tracks open document contents by intercepting `textDocument/didOpen`
//! and `textDocument/didChange` notifications.

use std::collections::HashMap;

use async_lsp::{
    lsp_types::{
        notification::{DidChangeTextDocument, DidOpenTextDocument},
        TextDocumentContentChangeEvent,
    },
    AnyNotification,
};
use tokio::sync::RwLock;

/// Cache of open document contents for tactic position detection.
pub struct DocumentCache {
    documents: RwLock<HashMap<String, String>>,
}

impl DocumentCache {
    pub fn new() -> Self {
        Self {
            documents: RwLock::new(HashMap::new()),
        }
    }

    /// Process an LSP notification, updating cache if it's a document event.
    pub async fn handle_notification(&self, notif: &AnyNotification) {
        if notif.method
            == <DidOpenTextDocument as async_lsp::lsp_types::notification::Notification>::METHOD
        {
            if let Ok(params) =
                serde_json::from_value::<async_lsp::lsp_types::DidOpenTextDocumentParams>(
                    notif.params.clone(),
                )
            {
                self.set(
                    params.text_document.uri.to_string(),
                    params.text_document.text,
                )
                .await;
            }
        } else if notif.method
            == <DidChangeTextDocument as async_lsp::lsp_types::notification::Notification>::METHOD
        {
            if let Ok(params) =
                serde_json::from_value::<async_lsp::lsp_types::DidChangeTextDocumentParams>(
                    notif.params.clone(),
                )
            {
                let uri = params.text_document.uri.to_string();
                self.apply_changes(&uri, &params.content_changes).await;
            }
        }
    }

    /// Set document content (from didOpen).
    async fn set(&self, uri: String, content: String) {
        let mut docs = self.documents.write().await;
        docs.insert(uri, content);
    }

    /// Apply incremental changes from didChange.
    async fn apply_changes(&self, uri: &str, changes: &[TextDocumentContentChangeEvent]) {
        let mut docs = self.documents.write().await;
        let Some(content) = docs.get_mut(uri) else {
            return;
        };

        for change in changes {
            if let Some(range) = change.range {
                // Incremental change: replace the specified range
                let start_offset =
                    line_char_to_offset(content, range.start.line, range.start.character);
                let end_offset =
                    line_char_to_offset(content, range.end.line, range.end.character);

                if start_offset <= content.len() && end_offset <= content.len() {
                    content.replace_range(start_offset..end_offset, &change.text);
                }
            } else {
                // Full document replacement
                *content = change.text.clone();
            }
        }
    }

    /// Get document content for tactic search.
    pub async fn get(&self, uri: &str) -> Option<String> {
        let docs = self.documents.read().await;
        docs.get(uri).cloned()
    }
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert line/character position to byte offset.
fn line_char_to_offset(content: &str, line: u32, character: u32) -> usize {
    let mut offset = 0;
    for (i, line_content) in content.lines().enumerate() {
        if i == line as usize {
            // Found the target line, add character offset
            // Note: LSP uses UTF-16 code units, but for simplicity we use byte offset
            // This may be slightly off for non-ASCII, but acceptable for line detection
            return offset + (character as usize).min(line_content.len());
        }
        offset += line_content.len() + 1; // +1 for newline
    }
    content.len()
}
