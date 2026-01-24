//! RPC client for communicating with Lean server via `$/lean/rpc/*` methods.

use std::collections::HashMap;

use async_lsp::ServerSocket;
use serde_json::json;
use tokio::sync::Mutex;
use tower_service::Service;

use super::{
    Goal, InteractiveGoalsResponse, RpcConnectResponse, GET_INTERACTIVE_GOALS, RPC_CALL,
    RPC_CONNECT,
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
    pub async fn connect(&self, uri: &str) -> Option<String> {
        let id = self.next_request_id().await;
        let params = json!({ "uri": uri });

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
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Vec<Goal>, String> {
        let session_id = {
            let sessions = self.sessions.lock().await;
            sessions.get(uri).map(|s| s.session_id.clone())
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
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
            "sessionId": session_id,
            "method": GET_INTERACTIVE_GOALS,
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
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
            line,
            character
        );

        let response = self.socket.clone().call(request).await.map_err(|e| {
            tracing::error!("RPC call failed: {e:?}");
            format!("RPC call failed: {e:?}")
        })?;

        Self::parse_goals_response(&response, uri, line, character)
    }

    fn parse_goals_response(
        response: &serde_json::Value,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Vec<Goal>, String> {
        if response.is_null() {
            tracing::debug!("No goals at {uri}:{line}:{character}");
            return Ok(vec![]);
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
}
