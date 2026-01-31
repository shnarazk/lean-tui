//! LSP service implementations for message interception and forwarding.

use std::{
    ops::ControlFlow,
    pin::Pin,
    result::Result as StdResult,
    sync::{Arc, OnceLock},
    task::{Context, Poll},
};

use async_lsp::{
    lsp_types::{
        notification::{DidChangeTextDocument, DidOpenTextDocument, Notification},
        DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    },
    AnyEvent, AnyNotification, AnyRequest, LspService,
};
use futures::Future;

use super::{cursor::extract_cursor_from_request, documents::DocumentCache};
use crate::{
    lean_rpc::RpcClient, proxy::goals::spawn_goal_fetch, tui_ipc::CursorInfo,
    tui_ipc::LspProxySocketEndpoint,
};

/// Spawn async task to forward didOpen to RPC client.
fn spawn_did_open(client: RpcClient, params: DidOpenTextDocumentParams) {
    tokio::spawn(async move {
        if let Err(e) = client.did_open(params).await {
            tracing::warn!("Failed to forward didOpen to RPC client: {e}");
        }
    });
}

/// Spawn async task to forward didChange to RPC client.
fn spawn_did_change(client: RpcClient, params: DidChangeTextDocumentParams) {
    tokio::spawn(async move {
        if let Err(e) = client.did_change(params).await {
            tracing::warn!("Failed to forward didChange to RPC client: {e}");
        }
    });
}

/// Parsed notification variants for single-parse optimization.
pub enum ParsedNotification {
    DidOpen(DidOpenTextDocumentParams),
    DidChange(DidChangeTextDocumentParams),
    Other,
}

impl ParsedNotification {
    /// Parse notification once, returning typed variant.
    fn from_any(notif: &AnyNotification) -> Self {
        if notif.method == DidOpenTextDocument::METHOD {
            serde_json::from_value(notif.params.clone()).map_or(Self::Other, Self::DidOpen)
        } else if notif.method == DidChangeTextDocument::METHOD {
            serde_json::from_value(notif.params.clone()).map_or(Self::Other, Self::DidChange)
        } else {
            Self::Other
        }
    }

    /// Extract cursor info from parsed notification.
    fn cursor_info(&self) -> Option<CursorInfo> {
        match self {
            Self::DidChange(params) => {
                let first_change = params.content_changes.first()?;
                let range = first_change.range?;
                Some(CursorInfo::new(
                    params.text_document.uri.clone(),
                    range.start,
                    "didChange",
                ))
            }
            _ => None,
        }
    }
}

/// Shared container for RPC client that can be set after service creation.
pub type RpcClientSlot = Arc<OnceLock<RpcClient>>;

/// Intercepts LSP messages, extracts cursor position, and forwards to inner
/// service.
pub struct InterceptService<S> {
    pub service: S,
    pub socket_server: Arc<LspProxySocketEndpoint>,
    pub document_cache: Arc<DocumentCache>,
    /// RPC client slot - set after client is initialized.
    pub rpc_client_slot: RpcClientSlot,
}

impl<S: LspService> InterceptService<S> {
    /// Broadcast cursor to TUI and fetch goals if RPC client is available.
    fn broadcast_cursor_and_fetch_goals(&self, cursor: &CursorInfo) {
        let _span = tracing::debug_span!(
            "cursor",
            file = cursor.filename().unwrap_or("?"),
            line = cursor.position.line,
            char = cursor.position.character
        )
        .entered();

        self.socket_server.broadcast_cursor(cursor.clone());

        if let Some(client) = self.rpc_client_slot.get() {
            spawn_goal_fetch(cursor, &self.socket_server, client);
        }
    }

    fn handle_request(&self, req: &AnyRequest) {
        if let Some(ref cursor) = extract_cursor_from_request(req) {
            self.broadcast_cursor_and_fetch_goals(cursor);
        }
    }

    fn handle_notification(&self, notif: &AnyNotification) {
        // Handle server-to-client notifications (`PublishDiagnostics`)
        DocumentCache::handle_server_notification(notif);

        // Parse once, use for all purposes
        let parsed = ParsedNotification::from_any(notif);

        // Update document cache with parsed data (no re-parsing)
        self.document_cache.handle_parsed_notification(&parsed);

        // Forward to RPC client with parsed data (no re-parsing)
        self.forward_parsed_to_rpc(&parsed);

        // Extract cursor from parsed data (no re-parsing)
        if let Some(ref cursor) = parsed.cursor_info() {
            self.broadcast_cursor_and_fetch_goals(cursor);
        }
    }

    fn forward_parsed_to_rpc(&self, parsed: &ParsedNotification) {
        let Some(client) = self.rpc_client_slot.get().cloned() else {
            return;
        };
        match parsed {
            ParsedNotification::DidOpen(params) => spawn_did_open(client, params.clone()),
            ParsedNotification::DidChange(params) => spawn_did_change(client, params.clone()),
            ParsedNotification::Other => {}
        }
    }
}

impl<S: LspService> tower_service::Service<AnyRequest> for InterceptService<S>
where
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = StdResult<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<StdResult<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: AnyRequest) -> Self::Future {
        self.handle_request(&req);
        let fut = self.service.call(req);
        Box::pin(fut)
    }
}

impl<S: LspService> LspService for InterceptService<S>
where
    S::Future: Send + 'static,
{
    fn notify(&mut self, notif: AnyNotification) -> ControlFlow<async_lsp::Result<()>> {
        self.handle_notification(&notif);
        self.service.notify(notif)
    }

    fn emit(&mut self, event: AnyEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.service.emit(event)
    }
}

/// Wrapper for deferred service initialization.
/// Used when the inner service isn't available at construction time.
pub struct DeferredService<S>(pub Option<S>);

impl<S: LspService> tower_service::Service<AnyRequest> for DeferredService<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<StdResult<(), Self::Error>> {
        self.0
            .as_mut()
            .expect("DeferredService must be initialized before use")
            .poll_ready(cx)
    }

    fn call(&mut self, req: AnyRequest) -> Self::Future {
        self.0
            .as_mut()
            .expect("DeferredService must be initialized before use")
            .call(req)
    }
}

impl<S: LspService> LspService for DeferredService<S> {
    fn notify(&mut self, notif: AnyNotification) -> ControlFlow<async_lsp::Result<()>> {
        tracing::debug!("DeferredService forwarding notification: {}", notif.method);
        self.0
            .as_mut()
            .expect("DeferredService must be initialized before use")
            .notify(notif)
    }

    fn emit(&mut self, event: AnyEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.0
            .as_mut()
            .expect("DeferredService must be initialized before use")
            .emit(event)
    }
}
