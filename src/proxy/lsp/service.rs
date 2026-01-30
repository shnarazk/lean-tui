//! LSP service implementations for message interception and forwarding.

use std::{
    ops::ControlFlow,
    pin::Pin,
    result::Result as StdResult,
    sync::{Arc, OnceLock},
    task::{Context, Poll},
};

use async_lsp::{AnyEvent, AnyNotification, AnyRequest, LspService};
use futures::Future;

use async_lsp::lsp_types::{
    notification::{DidChangeTextDocument, DidOpenTextDocument, Notification},
    DidChangeTextDocumentParams, DidOpenTextDocumentParams,
};

use super::{
    cursor::{extract_cursor_from_notification, extract_cursor_from_request},
    documents::DocumentCache,
};
use crate::{lean_rpc::LeanDagClient, proxy::goals::spawn_goal_fetch, tui_ipc::SocketServer};

/// Shared container for LeanDag client that can be set after service creation.
pub type LeanDagClientSlot = Arc<OnceLock<Arc<LeanDagClient>>>;

/// Intercepts LSP messages, extracts cursor position, and forwards to inner
/// service.
pub struct InterceptService<S> {
    pub service: S,
    pub socket_server: Arc<SocketServer>,
    pub document_cache: Arc<DocumentCache>,
    /// LeanDag client slot - set after lean-dag process is spawned.
    pub lean_dag_slot: LeanDagClientSlot,
}

impl<S: LspService> InterceptService<S> {
    /// Create with a shared document cache and LeanDag client.
    pub fn new(
        service: S,
        socket_server: Arc<SocketServer>,
        document_cache: Arc<DocumentCache>,
        lean_dag_slot: LeanDagClientSlot,
    ) -> Self {
        Self {
            service,
            socket_server,
            document_cache,
            lean_dag_slot,
        }
    }

    fn handle_request(&self, req: &AnyRequest) {
        if let Some(cursor) = extract_cursor_from_request(req) {
            let _span = tracing::debug_span!(
                "cursor",
                file = cursor.filename().unwrap_or("?"),
                line = cursor.position.line,
                char = cursor.position.character
            )
            .entered();

            self.socket_server.broadcast_cursor(cursor.clone());

            if let Some(lean_dag) = self.lean_dag_slot.get() {
                spawn_goal_fetch(&cursor, &self.socket_server, lean_dag);
            }
        }
    }

    fn handle_notification(&self, notif: &AnyNotification) {
        // Handle client-to-server notifications (DidOpen, DidChange)
        self.document_cache.handle_notification(notif);
        // Handle server-to-client notifications (PublishDiagnostics)
        self.document_cache.handle_server_notification(notif);

        // Forward document notifications to LeanDag client
        self.forward_to_lean_dag(notif);

        if let Some(cursor) = extract_cursor_from_notification(notif) {
            let _span = tracing::debug_span!(
                "cursor",
                file = cursor.filename().unwrap_or("?"),
                line = cursor.position.line,
                char = cursor.position.character
            )
            .entered();

            self.socket_server.broadcast_cursor(cursor.clone());

            if let Some(lean_dag) = self.lean_dag_slot.get() {
                spawn_goal_fetch(&cursor, &self.socket_server, lean_dag);
            }
        }
    }

    fn forward_to_lean_dag(&self, notif: &AnyNotification) {
        let Some(lean_dag) = self.lean_dag_slot.get() else {
            return;
        };

        if notif.method == DidOpenTextDocument::METHOD {
            if let Ok(params) =
                serde_json::from_value::<DidOpenTextDocumentParams>(notif.params.clone())
            {
                let lean_dag = lean_dag.clone();
                tokio::spawn(async move {
                    if let Err(e) = lean_dag.did_open(params).await {
                        tracing::warn!("Failed to forward didOpen to LeanDag: {}", e);
                    }
                });
            }
        } else if notif.method == DidChangeTextDocument::METHOD {
            if let Ok(params) =
                serde_json::from_value::<DidChangeTextDocumentParams>(notif.params.clone())
            {
                let lean_dag = lean_dag.clone();
                tokio::spawn(async move {
                    if let Err(e) = lean_dag.did_change(params).await {
                        tracing::warn!("Failed to forward didChange to LeanDag: {}", e);
                    }
                });
            }
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
