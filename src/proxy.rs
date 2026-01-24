use std::ops::ControlFlow;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_lsp::{AnyEvent, AnyNotification, AnyRequest, LspService, MainLoop};
use futures::Future;
use lsp_types::{
    DidChangeTextDocumentParams, TextDocumentPositionParams,
    notification::DidChangeTextDocument,
    request::{
        Completion, GotoDefinition, GotoTypeDefinition, GotoImplementation,
        HoverRequest, References, DocumentHighlightRequest, SignatureHelpRequest,
    },
};
use tokio::process::Command;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::cursor::{CursorBroadcaster, CursorInfo};
use crate::error::Result;

/// Forwards all LSP calls to an inner service.
struct Forward<S>(Option<S>);

impl<S: LspService> tower_service::Service<AnyRequest> for Forward<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        self.0.as_mut().unwrap().poll_ready(cx)
    }

    fn call(&mut self, req: AnyRequest) -> Self::Future {
        self.0.as_mut().unwrap().call(req)
    }
}

impl<S: LspService> LspService for Forward<S> {
    fn notify(&mut self, notif: AnyNotification) -> ControlFlow<async_lsp::Result<()>> {
        self.0.as_mut().unwrap().notify(notif)
    }

    fn emit(&mut self, event: AnyEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.0.as_mut().unwrap().emit(event)
    }
}

/// Intercepts LSP messages, extracts cursor position, and forwards to inner service.
struct Intercept<S> {
    service: S,
    direction: &'static str,
    broadcaster: Arc<CursorBroadcaster>,
}

impl<S: LspService> tower_service::Service<AnyRequest> for Intercept<S>
where
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = std::result::Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: AnyRequest) -> Self::Future {
        // Extract cursor position from position-containing requests
        if let Some(cursor) = extract_cursor(&req) {
            eprintln!(
                "[lean-tui] {} {}:{} ({})",
                cursor.filename(),
                cursor.line(),
                cursor.character(),
                cursor.method
            );
            self.broadcaster.broadcast_cursor(cursor);
        }

        let method = req.method.clone();
        let fut = self.service.call(req);
        let direction = self.direction;

        Box::pin(async move {
            let result = fut.await;
            tracing::debug!("{} response {}", direction, method);
            result
        })
    }
}

impl<S: LspService> LspService for Intercept<S>
where
    S::Future: Send + 'static,
{
    fn notify(&mut self, notif: AnyNotification) -> ControlFlow<async_lsp::Result<()>> {
        // Extract cursor from didChange notifications (insert mode live tracking)
        if let Some(cursor) = extract_cursor_from_notification(&notif) {
            eprintln!(
                "[lean-tui] {} {}:{} ({})",
                cursor.filename(),
                cursor.line(),
                cursor.character(),
                cursor.method
            );
            self.broadcaster.broadcast_cursor(cursor);
        }

        tracing::debug!("{} notification {}", self.direction, notif.method);
        self.service.notify(notif)
    }

    fn emit(&mut self, event: AnyEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.service.emit(event)
    }
}

/// Extract cursor position from textDocument/didChange notifications.
/// The edit position in insert mode represents the cursor location.
fn extract_cursor_from_notification(notif: &AnyNotification) -> Option<CursorInfo> {
    if notif.method != <DidChangeTextDocument as lsp_types::notification::Notification>::METHOD {
        return None;
    }

    let params: DidChangeTextDocumentParams = serde_json::from_value(notif.params.clone()).ok()?;
    let uri = params.text_document.uri.to_string();

    // Get the first content change - its range.start is the edit position
    // Range may be absent for full-document sync, but Helix uses incremental
    let first_change = params.content_changes.first()?;
    let range = first_change.range?;

    Some(CursorInfo::new(
        uri,
        range.start.line,
        range.start.character,
        "didChange",
    ))
}

/// Methods that contain TextDocumentPositionParams
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

/// Extract cursor position from LSP requests that contain TextDocumentPositionParams.
fn extract_cursor(req: &AnyRequest) -> Option<CursorInfo> {
    if !POSITION_METHODS.contains(&req.method.as_str()) {
        return None;
    }

    let params: TextDocumentPositionParams =
        serde_json::from_value(req.params.clone()).ok()?;

    Some(CursorInfo::new(
        params.text_document.uri.to_string(),
        params.position.line,
        params.position.character,
        &req.method,
    ))
}

pub async fn run() -> Result<()> {
    // Create broadcaster for TUI clients
    let broadcaster = Arc::new(CursorBroadcaster::new());
    broadcaster.clone().start_listener();

    // Spawn lake serve as child process
    let mut child = Command::new("lake")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let child_stdin = child.stdin.take().unwrap();
    let child_stdout = child.stdout.take().unwrap();

    // Create client connection to lake serve
    let broadcaster_client = broadcaster.clone();
    let (mut client_mainloop, server_socket) = MainLoop::new_client(|_| Intercept {
        service: Forward(None),
        direction: "<-",
        broadcaster: broadcaster_client,
    });

    // Create server connection from editor (stdin/stdout)
    let broadcaster_server = broadcaster.clone();
    let (server_mainloop, client_socket) = MainLoop::new_server(|_| Intercept {
        service: server_socket,
        direction: "->",
        broadcaster: broadcaster_server,
    });

    // Link the two sides
    client_mainloop.get_mut().service.0 = Some(client_socket);

    // Run both loops concurrently
    let client_task = tokio::spawn(async move {
        client_mainloop
            .run_buffered(child_stdout.compat(), child_stdin.compat_write())
            .await
    });

    let server_task = tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        server_mainloop
            .run_buffered(stdin.compat(), stdout.compat_write())
            .await
    });

    tokio::select! {
        result = client_task => {
            let inner = result.map_err(|e| crate::error::Error::Lsp(e.to_string()))?;
            inner.map_err(|e| crate::error::Error::Lsp(e.to_string()))?;
        }
        result = server_task => {
            let inner = result.map_err(|e| crate::error::Error::Lsp(e.to_string()))?;
            inner.map_err(|e| crate::error::Error::Lsp(e.to_string()))?;
        }
    }

    Ok(())
}
