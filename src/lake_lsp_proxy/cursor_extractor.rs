//! Cursor position extraction from LSP messages.

use async_lsp::lsp_types::{
    self,
    notification::DidChangeTextDocument,
    request::{
        Completion, DocumentHighlightRequest, GotoDefinition, GotoImplementation,
        GotoTypeDefinition, HoverRequest, References, SignatureHelpRequest,
    },
};
use async_lsp::{AnyNotification, AnyRequest};

use crate::tui_ipc::CursorInfo;

/// LSP methods that contain `TextDocumentPositionParams`
const POSITION_METHODS: &[&str] = &[
    <HoverRequest as lsp_types::request::Request>::METHOD,
    <GotoDefinition as lsp_types::request::Request>::METHOD,
    <GotoTypeDefinition as lsp_types::request::Request>::METHOD,
    <GotoImplementation as lsp_types::request::Request>::METHOD,
    <References as lsp_types::request::Request>::METHOD,
    <DocumentHighlightRequest as lsp_types::request::Request>::METHOD,
    <SignatureHelpRequest as lsp_types::request::Request>::METHOD,
    <Completion as lsp_types::request::Request>::METHOD,
];

/// Extract cursor position from LSP requests that contain `TextDocumentPositionParams`.
pub fn extract_cursor(req: &AnyRequest) -> Option<CursorInfo> {
    POSITION_METHODS
        .contains(&req.method.as_str())
        .then(|| {
            let params: lsp_types::TextDocumentPositionParams =
                serde_json::from_value(req.params.clone()).ok()?;

            Some(CursorInfo::new(
                params.text_document.uri.to_string(),
                params.position.line,
                params.position.character,
                &req.method,
            ))
        })
        .flatten()
}

/// Extract cursor position from `textDocument/didChange` notifications.
/// The edit position in insert mode represents the cursor location.
pub fn extract_cursor_from_notification(notif: &AnyNotification) -> Option<CursorInfo> {
    (notif.method == <DidChangeTextDocument as lsp_types::notification::Notification>::METHOD)
        .then(|| {
            let params: lsp_types::DidChangeTextDocumentParams =
                serde_json::from_value(notif.params.clone()).ok()?;

            let uri = params.text_document.uri.to_string();
            let first_change = params.content_changes.first()?;
            let range = first_change.range?;

            Some(CursorInfo::new(
                uri,
                range.start.line,
                range.start.character,
                "didChange",
            ))
        })
        .flatten()
}
