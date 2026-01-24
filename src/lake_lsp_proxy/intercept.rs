//! LSP service interceptor for cursor tracking and goal fetching.

use std::ops::ControlFlow;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_lsp::{AnyEvent, AnyNotification, AnyRequest, LspService};
use futures::Future;

use crate::lake_ipc::{spawn_goal_fetch, RpcClient};
use crate::tui_ipc::Broadcaster;

use super::cursor_extractor::{extract_cursor, extract_cursor_from_notification};

/// Intercepts LSP messages, extracts cursor position, and forwards to inner service.
pub struct Intercept<S> {
    pub service: S,
    pub direction: &'static str,
    pub broadcaster: Arc<Broadcaster>,
    pub rpc_client: Option<Arc<RpcClient>>,
}

impl<S: LspService> Intercept<S> {
    pub fn new(
        service: S,
        direction: &'static str,
        broadcaster: Arc<Broadcaster>,
        rpc_client: Option<Arc<RpcClient>>,
    ) -> Self {
        Self {
            service,
            direction,
            broadcaster,
            rpc_client,
        }
    }

    fn handle_request(&self, req: &AnyRequest) {
        extract_cursor(req).iter().for_each(|cursor| {
            let _span = tracing::info_span!(
                "cursor",
                file = cursor.filename(),
                line = cursor.line(),
                char = cursor.character()
            )
            .entered();
            tracing::info!("cursor position");

            self.broadcaster.broadcast_cursor(cursor.clone());

            if let Some(rpc) = &self.rpc_client {
                spawn_goal_fetch(cursor, &self.broadcaster, rpc);
            }
        });
    }

    fn handle_notification(&self, notif: &AnyNotification) {
        extract_cursor_from_notification(notif).iter().for_each(|cursor| {
            let _span = tracing::info_span!(
                "cursor",
                file = cursor.filename(),
                line = cursor.line(),
                char = cursor.character()
            )
            .entered();
            tracing::info!("cursor position");

            self.broadcaster.broadcast_cursor(cursor.clone());

            if let Some(rpc) = &self.rpc_client {
                spawn_goal_fetch(cursor, &self.broadcaster, rpc);
            }
        });
    }
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
        self.handle_request(&req);

        let method = req.method.clone();
        let fut = self.service.call(req);
        let direction = self.direction;

        Box::pin(async move {
            let result = fut.await;
            tracing::debug!("{direction} response {method}");
            result
        })
    }
}

impl<S: LspService> LspService for Intercept<S>
where
    S::Future: Send + 'static,
{
    fn notify(&mut self, notif: AnyNotification) -> ControlFlow<async_lsp::Result<()>> {
        self.handle_notification(&notif);
        tracing::debug!("{} notification {}", self.direction, notif.method);
        self.service.notify(notif)
    }

    fn emit(&mut self, event: AnyEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.service.emit(event)
    }
}
