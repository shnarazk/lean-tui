//! RPC client for communicating with Lean server via `$/lean/rpc/*` methods.

use std::collections::HashMap;

use async_lsp::{lsp_types, ServerSocket};
use lsp_types::{LocationLink, Position, TextDocumentIdentifier, Url};
use serde_json::json;
use tokio::sync::Mutex;
use tower_service::Service;

use super::{
    Goal, InteractiveGoalsResponse, RpcConnectResponse, GET_GO_TO_LOCATION, GET_INTERACTIVE_GOALS,
    RPC_CALL, RPC_CONNECT,
};

/// Session state for a single file
struct Session {
    session_id: String,
}

/// RPC client that manages sessions and sends requests to lake serve.
pub struct RpcClient {
    socket: ServerSocket,
    sessions: Mutex<HashMap<String, Session>>,
    next_id: Mutex<i64>,
}

impl RpcClient {
    pub fn new(socket: ServerSocket) -> Self {
        Self {
            socket,
            sessions: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1000), // Start at 1000 to avoid conflicts with editor
        }
    }

    async fn next_request_id(&self) -> i64 {
        let mut id = self.next_id.lock().await;
        let current = *id;
        *id += 1;
        current
    }

    /// Connect to RPC for a given file URI. Returns session ID on success.
    pub async fn connect(&self, uri: &Url) -> Option<String> {
        let id = self.next_request_id().await;
        let params = json!({ "uri": uri.as_str() });

        tracing::info!("RPC connect for {}", uri);

        // Construct request via JSON to work around non-exhaustive struct
        let request_json = json!({
            "id": id,
            "method": RPC_CONNECT,
            "params": params
        });
        let request: async_lsp::AnyRequest = serde_json::from_value(request_json).ok()?;

        match self.socket.clone().call(request).await {
            Ok(response) => {
                tracing::debug!("RPC connect response: {}", response);
                match serde_json::from_value::<RpcConnectResponse>(response.clone()) {
                    Ok(resp) => {
                        let session_id = resp.session_id.clone();
                        tracing::info!("RPC session: {}", session_id);

                        let mut sessions = self.sessions.lock().await;
                        sessions.insert(
                            uri.to_string(),
                            Session {
                                session_id: session_id.clone(),
                            },
                        );
                        drop(sessions);

                        // Start keepalive task
                        Self::start_keepalive();

                        Some(session_id)
                    }
                    Err(e) => {
                        tracing::error!("RPC connect parse error: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                tracing::error!("RPC connect error: {:?}", e);
                None
            }
        }
    }

    const fn start_keepalive() {
        // TODO: Implement keepalive once we figure out how to send raw
        // notifications with ServerSocket. For now, sessions will time
        // out after ~60s without keepalive, but we'll reconnect
        // automatically when that happens.
        //
        // The issue is that ServerSocket::notify expects a typed Notification,
        // but Lean's $/lean/rpc/keepAlive is a custom method not in lsp-types.
    }

    /// Get interactive goals at a position.
    pub async fn get_goals(
        &self,
        text_document: &TextDocumentIdentifier,
        position: Position,
    ) -> Result<Vec<Goal>, String> {
        let uri = &text_document.uri;
        let session_id = {
            let sessions = self.sessions.lock().await;
            sessions.get(uri.as_str()).map(|s| s.session_id.clone())
        };

        let session_id = match session_id {
            Some(id) => id,
            None => {
                // Try to connect first
                match self.connect(uri).await {
                    Some(id) => id,
                    None => return Err("Failed to connect RPC session".to_string()),
                }
            }
        };

        let id = self.next_request_id().await;
        // Structure from lean.nvim: textDocument/position at top level AND inside
        // params See: https://github.com/Julian/lean.nvim/blob/main/lua/lean/rpc.lua#L183-L186
        let params = json!({
            "textDocument": text_document,
            "position": position,
            "sessionId": session_id,
            "method": GET_INTERACTIVE_GOALS,
            "params": {
                "textDocument": text_document,
                "position": position
            }
        });

        let request_json = json!({
            "id": id,
            "method": RPC_CALL,
            "params": params
        });
        let request: async_lsp::AnyRequest = match serde_json::from_value(request_json) {
            Ok(r) => r,
            Err(e) => return Err(format!("Failed to construct RPC request: {e}")),
        };

        tracing::debug!(
            "Calling getInteractiveGoals at {}:{}:{}",
            uri,
            position.line,
            position.character
        );

        let response = self.socket.clone().call(request).await.map_err(|e| {
            tracing::error!("RPC call failed: {e:?}");
            format!("RPC call failed: {e:?}")
        })?;

        Self::parse_goals_response(&response, uri, position)
    }

    fn parse_goals_response(
        response: &serde_json::Value,
        uri: &Url,
        position: Position,
    ) -> Result<Vec<Goal>, String> {
        if response.is_null() {
            tracing::debug!(
                "No goals at {uri}:{0}:{1}",
                position.line,
                position.character
            );
            return Ok(vec![]);
        }

        // Log raw hypothesis structure for debugging go-to-definition
        if let Some(goals) = response.get("goals").and_then(|g| g.as_array()) {
            for (i, goal) in goals.iter().enumerate() {
                if let Some(hyps) = goal.get("hyps").and_then(|h| h.as_array()) {
                    for (j, hyp) in hyps.iter().enumerate() {
                        tracing::debug!(
                            "Goal {i} Hyp {j} raw: {}",
                            serde_json::to_string_pretty(hyp).unwrap_or_default()
                        );
                    }
                }
            }
        }

        let resp: InteractiveGoalsResponse =
            serde_json::from_value(response.clone()).map_err(|e| {
                tracing::error!("Failed to parse goals response: {e}");
                tracing::debug!("Raw response: {response}");
                format!("Failed to parse goals: {e}")
            })?;

        let goals = resp.to_goals();
        if !goals.is_empty() {
            tracing::info!("Found {} goal(s)", goals.len());
        }
        Ok(goals)
    }

    /// Get the go-to-definition location for an `InfoWithCtx` reference.
    ///
    /// This calls `Lean.Widget.getGoToLocation` with the info extracted from
    /// a hypothesis's `CodeWithInfos` type field.
    ///
    /// Returns a `LocationLink` on success.
    pub async fn get_go_to_location(
        &self,
        text_document: &TextDocumentIdentifier,
        position: Position,
        info: serde_json::Value,
    ) -> Result<Option<LocationLink>, String> {
        let uri = &text_document.uri;
        let session_id = {
            let sessions = self.sessions.lock().await;
            sessions.get(uri.as_str()).map(|s| s.session_id.clone())
        };

        let session_id = match session_id {
            Some(id) => id,
            None => {
                // Try to connect first
                match self.connect(uri).await {
                    Some(id) => id,
                    None => return Err("Failed to connect RPC session".to_string()),
                }
            }
        };

        let id = self.next_request_id().await;
        let params = json!({
            "textDocument": text_document,
            "position": position,
            "sessionId": session_id,
            "method": GET_GO_TO_LOCATION,
            "params": {
                "kind": "definition",
                "info": info
            }
        });

        let request_json = json!({
            "id": id,
            "method": RPC_CALL,
            "params": params
        });
        let request: async_lsp::AnyRequest = match serde_json::from_value(request_json) {
            Ok(r) => r,
            Err(e) => return Err(format!("Failed to construct RPC request: {e}")),
        };

        tracing::debug!("Calling getGoToLocation for info");

        let response = self.socket.clone().call(request).await.map_err(|e| {
            tracing::error!("getGoToLocation RPC call failed: {e:?}");
            format!("RPC call failed: {e:?}")
        })?;

        Self::parse_location_response(&response)
    }

    fn parse_location_response(
        response: &serde_json::Value,
    ) -> Result<Option<LocationLink>, String> {
        // Response is an array of LocationLink objects
        if response.is_null() {
            tracing::debug!("No location found");
            return Ok(None);
        }

        let locations: Vec<LocationLink> =
            serde_json::from_value(response.clone()).map_err(|e| {
                tracing::debug!("Failed to parse LocationLink array: {e}, response: {response}");
                format!("Failed to parse location response: {e}")
            })?;

        if locations.is_empty() {
            tracing::debug!("Empty location array");
            return Ok(None);
        }

        let loc = &locations[0];
        // Use target_selection_range for logging (more precise location)
        tracing::info!(
            "Found location: {}:{}:{}",
            loc.target_uri,
            loc.target_selection_range.start.line,
            loc.target_selection_range.start.character
        );
        Ok(Some(loc.clone()))
    }
}
