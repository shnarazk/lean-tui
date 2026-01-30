//! Document content cache for tracking open documents.

use std::{collections::HashMap, sync::Mutex};

use async_lsp::{
    lsp_types::{
        notification::{
            DidChangeTextDocument, DidOpenTextDocument, Notification, PublishDiagnostics,
        },
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, Position, PublishDiagnosticsParams,
        TextDocumentContentChangeEvent,
    },
    AnyNotification,
};

pub struct DocumentCache {
    documents: Mutex<HashMap<String, String>>,
}

impl DocumentCache {
    pub fn new() -> Self {
        Self {
            documents: Mutex::new(HashMap::new()),
        }
    }

    /// Handle client-to-server notifications (`DidOpen`, `DidChange`).
    pub fn handle_notification(&self, notif: &AnyNotification) {
        if notif.method == DidOpenTextDocument::METHOD {
            let Ok(p) = serde_json::from_value::<DidOpenTextDocumentParams>(notif.params.clone())
            else {
                return;
            };
            let uri = p.text_document.uri.as_str();
            tracing::debug!("DidOpen URI: {uri}");
            self.update(uri, p.text_document.text);
        } else if notif.method == DidChangeTextDocument::METHOD {
            let Ok(p) = serde_json::from_value::<DidChangeTextDocumentParams>(notif.params.clone())
            else {
                return;
            };
            let uri = p.text_document.uri.as_str();
            tracing::debug!("DidChange URI: {uri}");
            if let Some(content) = self.apply_changes(uri, &p.content_changes) {
                self.update(uri, content);
            }
        }
    }

    /// Handle server-to-client notifications (`PublishDiagnostics`).
    pub fn handle_server_notification(&self, notif: &AnyNotification) {
        if notif.method == PublishDiagnostics::METHOD {
            let Ok(p) = serde_json::from_value::<PublishDiagnosticsParams>(notif.params.clone())
            else {
                return;
            };
            tracing::debug!(
                "PublishDiagnostics for {}: {} diagnostics",
                p.uri,
                p.diagnostics.len()
            );
        }
    }

    fn update(&self, uri: &str, content: String) {
        self.documents
            .lock()
            .expect("lock poisoned")
            .insert(uri.to_string(), content);
    }

    fn apply_changes(
        &self,
        uri: &str,
        changes: &[TextDocumentContentChangeEvent],
    ) -> Option<String> {
        let docs = self.documents.lock().expect("lock poisoned");
        let mut content = docs.get(uri)?.clone();
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
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self::new()
    }
}

fn position_to_offset(content: &str, pos: Position) -> usize {
    content
        .lines()
        .take(pos.line as usize)
        .map(|line| line.len() + 1)
        .sum::<usize>()
        + pos.character as usize
}
