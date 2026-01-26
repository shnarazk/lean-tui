//! Cursor position extraction from LSP messages.

use async_lsp::{
    lsp_types::{
        notification::{DidChangeTextDocument, Notification},
        request::{
            Completion, DocumentHighlightRequest, GotoDefinition, GotoImplementation,
            GotoTypeDefinition, HoverRequest, References, Request, SignatureHelpRequest,
        },
        DidChangeTextDocumentParams, TextDocumentPositionParams,
    },
    AnyNotification, AnyRequest,
};

use crate::tui_ipc::CursorInfo;

const POSITION_METHODS: &[&str] = &[
    <HoverRequest as Request>::METHOD,
    <GotoDefinition as Request>::METHOD,
    <GotoTypeDefinition as Request>::METHOD,
    <GotoImplementation as Request>::METHOD,
    <References as Request>::METHOD,
    <DocumentHighlightRequest as Request>::METHOD,
    <SignatureHelpRequest as Request>::METHOD,
    <Completion as Request>::METHOD,
];

pub fn extract_cursor_from_request(req: &AnyRequest) -> Option<CursorInfo> {
    if !POSITION_METHODS.contains(&req.method.as_str()) {
        return None;
    }
    let params: TextDocumentPositionParams = serde_json::from_value(req.params.clone()).ok()?;
    Some(CursorInfo::new(
        params.text_document.uri,
        params.position,
        &req.method,
    ))
}

pub fn extract_cursor_from_notification(notif: &AnyNotification) -> Option<CursorInfo> {
    if notif.method != <DidChangeTextDocument as Notification>::METHOD {
        return None;
    }
    let params: DidChangeTextDocumentParams = serde_json::from_value(notif.params.clone()).ok()?;
    let first_change = params.content_changes.first()?;
    let range = first_change.range?;
    Some(CursorInfo::new(
        params.text_document.uri,
        range.start,
        "didChange",
    ))
}
