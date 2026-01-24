//! RPC client for communicating with Lean server via `$/lean/rpc/*` methods.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicI64, Ordering},
};

use async_lsp::{lsp_types, ServerSocket};
use lsp_types::{LocationLink, Position, TextDocumentIdentifier, Url};
use serde::Serialize;
use serde_json::json;
use tokio::sync::Mutex;
use tower_service::Service;

use super::{
    Goal, InteractiveGoalsResponse, RpcConnectResponse, GET_GOTO_LOCATION, GET_INTERACTIVE_GOALS,
    RPC_CALL, RPC_CONNECT,
};
use crate::error::LspError;

/// Lean RPC connect request parameters.
#[derive(Serialize)]
struct RpcConnectParams {
    uri: String,
}

/// Lean RPC call wrapper parameters.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RpcCallParams<P: Serialize> {
    text_document: TextDocumentIdentifier,
    position: Position,
    session_id: String,
    method: &'static str,
    params: P,
}

/// Parameters for `getInteractiveGoals` inner call.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GetInteractiveGoalsParams {
    text_document: TextDocumentIdentifier,
    position: Position,
}

/// Navigation target kind for `Lean.Widget.getGoToLocation`.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GoToKind {
    /// Navigate to the definition site.
    Definition,
}

/// Parameters for `Lean.Widget.getGoToLocation` inner call.
#[derive(Serialize, Clone)]
struct GetGoToLocationParams {
    kind: GoToKind,
    info: serde_json::Value,
}

/// RPC client that manages sessions and sends requests to lake serve.
pub struct RpcClient {
    socket: ServerSocket,
    /// Maps file URI to session ID.
    sessions: Mutex<HashMap<String, String>>,
    next_id: AtomicI64,
}

impl RpcClient {
    pub fn new(socket: ServerSocket) -> Self {
        Self {
            socket,
            sessions: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1000),
        }
    }

    fn next_request_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send an LSP request and return the response.
    async fn request(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<serde_json::Value, LspError> {
        let id = self.next_request_id();
        let request_json = json!({ "id": id, "method": method, "params": params });
        let request: async_lsp::AnyRequest = serde_json::from_value(request_json)
            .map_err(|e| LspError::InvalidRequest(e.to_string()))?;

        self.socket
            .clone()
            .call(request)
            .await
            .map_err(|e| Self::parse_rpc_error(&format!("{e:?}")))
    }

    /// Parse an RPC error string into a structured `LspError`.
    fn parse_rpc_error(error: &str) -> LspError {
        if error.contains("Outdated RPC session") || error.contains("-32900") {
            LspError::SessionExpired
        } else {
            LspError::RpcError {
                code: None,
                message: error.to_string(),
            }
        }
    }

    /// Connect to RPC for a given file URI. Returns session ID on success.
    async fn connect(&self, uri: &Url) -> Result<String, LspError> {
        let params = RpcConnectParams {
            uri: uri.to_string(),
        };
        let response = self.request(RPC_CONNECT, params).await?;

        let resp: RpcConnectResponse =
            serde_json::from_value(response).map_err(|e| LspError::ParseError(e.to_string()))?;

        let session_id = resp.session_id;
        tracing::info!("RPC session for {uri}: {session_id}");

        self.sessions
            .lock()
            .await
            .insert(uri.to_string(), session_id.clone());
        Ok(session_id)
    }

    /// Get or create a session for a URI.
    async fn get_session(&self, uri: &Url) -> Result<String, LspError> {
        if let Some(id) = self.sessions.lock().await.get(uri.as_str()).cloned() {
            return Ok(id);
        }
        self.connect(uri).await
    }

    /// Invalidate the session for a given URI.
    async fn invalidate_session(&self, uri: &Url) {
        self.sessions.lock().await.remove(uri.as_str());
    }

    /// Call a Lean RPC method with automatic session management.
    async fn rpc_call<P: Serialize>(
        &self,
        uri: &Url,
        text_document: &TextDocumentIdentifier,
        position: Position,
        method: &'static str,
        inner_params: P,
    ) -> Result<serde_json::Value, LspError> {
        let session_id = self.get_session(uri).await?;

        let params = RpcCallParams {
            text_document: text_document.clone(),
            position,
            session_id,
            method,
            params: inner_params,
        };

        self.request(RPC_CALL, params).await
    }

    /// Call a Lean RPC method with automatic session retry on expiry.
    async fn rpc_call_with_retry<P: Serialize + Clone>(
        &self,
        uri: &Url,
        text_document: &TextDocumentIdentifier,
        position: Position,
        method: &'static str,
        params: P,
    ) -> Result<serde_json::Value, LspError> {
        let result = self
            .rpc_call(uri, text_document, position, method, params.clone())
            .await;

        match result {
            Ok(r) => Ok(r),
            Err(LspError::SessionExpired) => {
                tracing::info!("Session expired, reconnecting...");
                self.invalidate_session(uri).await;
                self.rpc_call(uri, text_document, position, method, params)
                    .await
            }
            Err(e) => Err(e),
        }
    }

    /// Get interactive goals at a position. Retries once if session expired.
    pub async fn get_goals(
        &self,
        text_document: &TextDocumentIdentifier,
        position: Position,
    ) -> Result<Vec<Goal>, LspError> {
        let uri = &text_document.uri;
        let params = GetInteractiveGoalsParams {
            text_document: text_document.clone(),
            position,
        };

        let response = self
            .rpc_call_with_retry(uri, text_document, position, GET_INTERACTIVE_GOALS, params)
            .await?;

        Self::parse_goals_response(&response)
    }

    /// Get location using Lean's widget RPC (for hypothesis navigation).
    /// Uses `Lean.Widget.getGoToLocation` with an `InfoWithCtx` reference.
    pub async fn get_goto_location(
        &self,
        text_document: &TextDocumentIdentifier,
        position: Position,
        kind: GoToKind,
        info: serde_json::Value,
    ) -> Result<Option<LocationLink>, LspError> {
        let uri = &text_document.uri;
        let params = GetGoToLocationParams { kind, info };

        let response = self
            .rpc_call_with_retry(uri, text_document, position, GET_GOTO_LOCATION, params)
            .await?;

        Ok(Self::parse_definition_response(&response))
    }

    fn parse_goals_response(response: &serde_json::Value) -> Result<Vec<Goal>, LspError> {
        if response.is_null() {
            return Ok(vec![]);
        }

        let resp: InteractiveGoalsResponse = serde_json::from_value(response.clone())
            .map_err(|e| LspError::ParseError(e.to_string()))?;

        let goals = resp.to_goals();
        if !goals.is_empty() {
            tracing::info!("Found {} goal(s)", goals.len());
        }
        Ok(goals)
    }

    /// Parse `getGoToLocation` response (`LocationLink[]`).
    fn parse_definition_response(response: &serde_json::Value) -> Option<LocationLink> {
        if response.is_null() {
            return None;
        }

        // Try LocationLink[]
        if let Ok(locs) = serde_json::from_value::<Vec<LocationLink>>(response.clone()) {
            if let Some(loc) = locs.into_iter().next() {
                tracing::info!(
                    "Definition: {}:{}",
                    loc.target_uri,
                    loc.target_selection_range.start.line
                );
                return Some(loc);
            }
        }

        // Try Location[]
        if let Ok(locs) = serde_json::from_value::<Vec<lsp_types::Location>>(response.clone()) {
            if let Some(loc) = locs.into_iter().next() {
                tracing::info!("Definition: {}:{}", loc.uri, loc.range.start.line);
                return Some(LocationLink {
                    origin_selection_range: None,
                    target_uri: loc.uri,
                    target_range: loc.range,
                    target_selection_range: loc.range,
                });
            }
        }

        // Try single Location
        if let Ok(loc) = serde_json::from_value::<lsp_types::Location>(response.clone()) {
            tracing::info!("Definition: {}:{}", loc.uri, loc.range.start.line);
            return Some(LocationLink {
                origin_selection_range: None,
                target_uri: loc.uri,
                target_range: loc.range,
                target_selection_range: loc.range,
            });
        }

        None
    }
}
