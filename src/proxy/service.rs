//! LSP service implementations for message interception and forwarding.

use std::{
    ops::ControlFlow,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use async_lsp::{AnyEvent, AnyNotification, AnyRequest, LspService};
use futures::Future;

use super::{
    cursor::{extract_cursor_from_notification, extract_cursor_from_request},
    documents::DocumentCache,
    goals::spawn_goal_fetch,
};
use crate::{lean_rpc::RpcClient, tui_ipc::SocketServer};

/// Intercepts LSP messages, extracts cursor position, and forwards to inner
/// service.
pub struct InterceptService<S> {
    pub service: S,
    pub socket_server: Arc<SocketServer>,
    pub rpc_client: Option<Arc<RpcClient>>,
    pub document_cache: Arc<DocumentCache>,
}

impl<S: LspService> InterceptService<S> {
    #[allow(dead_code)]
    pub fn new(
        service: S,
        socket_server: Arc<SocketServer>,
        rpc_client: Option<Arc<RpcClient>>,
    ) -> Self {
        Self {
            service,
            socket_server,
            rpc_client,
            document_cache: Arc::new(DocumentCache::new()),
        }
    }

    /// Create with a shared document cache (for sharing between client/server sides).
    pub fn with_document_cache(
        service: S,
        socket_server: Arc<SocketServer>,
        rpc_client: Option<Arc<RpcClient>>,
        document_cache: Arc<DocumentCache>,
    ) -> Self {
        Self {
            service,
            socket_server,
            rpc_client,
            document_cache,
        }
    }

    fn handle_request(&self, req: &AnyRequest) {
        if let Some(cursor) = extract_cursor_from_request(req) {
            let _span = tracing::info_span!(
                "cursor",
                file = cursor.filename(),
                line = cursor.line(),
                char = cursor.character()
            )
            .entered();
            tracing::info!("cursor position");

            self.socket_server.broadcast_cursor(cursor.clone());

            if let Some(rpc) = &self.rpc_client {
                spawn_goal_fetch(&cursor, &self.socket_server, rpc);
            }
        }
    }

    fn handle_notification(&self, notif: &AnyNotification) {
        // Track document content for tactic position detection
        let doc_cache = self.document_cache.clone();
        let notif_clone = notif.clone();
        tokio::spawn(async move {
            doc_cache.handle_notification(&notif_clone).await;
        });

        if let Some(cursor) = extract_cursor_from_notification(notif) {
            let _span = tracing::info_span!(
                "cursor",
                file = cursor.filename(),
                line = cursor.line(),
                char = cursor.character()
            )
            .entered();
            tracing::info!("cursor position");

            self.socket_server.broadcast_cursor(cursor.clone());

            if let Some(rpc) = &self.rpc_client {
                spawn_goal_fetch(&cursor, &self.socket_server, rpc);
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
    type Future = Pin<Box<dyn Future<Output = std::result::Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
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

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
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
