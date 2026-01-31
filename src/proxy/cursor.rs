//! Cursor position extraction from LSP messages.

use async_lsp::{
    lsp_types::{
        request::{
            Completion, DocumentHighlightRequest, GotoDefinition, GotoImplementation,
            GotoTypeDefinition, HoverRequest, References, Request, SignatureHelpRequest,
        },
        TextDocumentPositionParams,
    },
    AnyRequest,
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
