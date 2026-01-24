//! Forward wrapper for deferred service initialization.

use std::ops::ControlFlow;
use std::task::{Context, Poll};

use async_lsp::{AnyEvent, AnyNotification, AnyRequest, LspService};

/// Forwards all LSP calls to an inner service.
/// Used for deferred initialization of the client socket.
pub struct Forward<S>(pub Option<S>);

impl<S: LspService> tower_service::Service<AnyRequest> for Forward<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        self.0
            .as_mut()
            .expect("Forward service must be initialized")
            .poll_ready(cx)
    }

    fn call(&mut self, req: AnyRequest) -> Self::Future {
        self.0
            .as_mut()
            .expect("Forward service must be initialized")
            .call(req)
    }
}

impl<S: LspService> LspService for Forward<S> {
    fn notify(&mut self, notif: AnyNotification) -> ControlFlow<async_lsp::Result<()>> {
        self.0
            .as_mut()
            .expect("Forward service must be initialized")
            .notify(notif)
    }

    fn emit(&mut self, event: AnyEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.0
            .as_mut()
            .expect("Forward service must be initialized")
            .emit(event)
    }
}
