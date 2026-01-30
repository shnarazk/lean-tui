//! RPC client for Lean server communication.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicI64, Ordering},
};

use async_lsp::{
    lsp_types::{Location, LocationLink, Position, TextDocumentIdentifier, Url},
    AnyRequest, ServerSocket,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tower_service::Service;

use super::{
    Goal, GotoLocation, InteractiveGoalsResponse, InteractiveTermGoalResponse, RpcConnectResponse,
    GET_GOTO_LOCATION, GET_INTERACTIVE_GOALS, GET_INTERACTIVE_TERM_GOAL, GET_PROOF_DAG, RPC_CALL,
    RPC_CONNECT,
};
use crate::error::LspError;
use super::dag::ProofDag;

#[derive(Serialize)]
struct RpcConnectParams {
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RpcCallParams<P> {
    text_document: TextDocumentIdentifier,
    position: Position,
    session_id: String,
    method: &'static str,
    params: P,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GetInteractiveGoalsParams {
    text_document: TextDocumentIdentifier,
    position: Position,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GoToKind {
    Definition,
    Type,
}

#[derive(Serialize, Clone)]
struct GetGoToLocationParams {
    kind: GoToKind,
    info: serde_json::Value,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GetProofDagParams {
    text_document: TextDocumentIdentifier,
    position: Position,
    mode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetProofDagResult {
    proof_dag: ProofDag,
    #[allow(dead_code)]
    version: u32,
}

pub struct RpcClient {
    socket: ServerSocket,
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

    async fn request(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<serde_json::Value, LspError> {
        let id = self.next_request_id();
        let request_json = json!({ "id": id, "method": method, "params": params });
        let request: AnyRequest = serde_json::from_value(request_json)
            .map_err(|e| LspError::InvalidRequest(e.to_string()))?;

        self.socket
            .clone()
            .call(request)
            .await
            .map_err(|e| Self::parse_rpc_error(&format!("{e:?}")))
    }

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

    async fn get_session(&self, uri: &Url) -> Result<String, LspError> {
        let existing = self.sessions.lock().await.get(uri.as_str()).cloned();
        if let Some(id) = existing {
            return Ok(id);
        }
        self.connect(uri).await
    }

    async fn invalidate_session(&self, uri: &Url) {
        self.sessions.lock().await.remove(uri.as_str());
    }

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

    pub async fn get_term_goal(
        &self,
        text_document: &TextDocumentIdentifier,
        position: Position,
    ) -> Result<Option<Goal>, LspError> {
        let uri = &text_document.uri;
        let params = GetInteractiveGoalsParams {
            text_document: text_document.clone(),
            position,
        };

        let response = self
            .rpc_call_with_retry(
                uri,
                text_document,
                position,
                GET_INTERACTIVE_TERM_GOAL,
                params,
            )
            .await?;

        if response.is_null() {
            return Ok(None);
        }

        let resp: InteractiveTermGoalResponse =
            serde_json::from_value(response).map_err(|e| LspError::ParseError(e.to_string()))?;

        Ok(Some(resp.to_goal()))
    }

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

    /// Resolve goto locations for all hypotheses and target in a goal.
    /// This pre-fetches locations so navigation doesn't depend on RPC session.
    pub async fn resolve_goto_locations(
        &self,
        text_document: &TextDocumentIdentifier,
        position: Position,
        goal: &mut Goal,
    ) {
        // Resolve target locations (both definition and type)
        if let Some(info) = goal.target.first_info() {
            goal.goto_locations.definition = self
                .resolve_single_location(
                    text_document,
                    position,
                    GoToKind::Definition,
                    info.clone(),
                )
                .await;
            goal.goto_locations.type_def = self
                .resolve_single_location(text_document, position, GoToKind::Type, info)
                .await;
        }

        // Resolve hypothesis locations (both definition and type)
        for hyp in &mut goal.hyps {
            if let Some(info) = hyp.first_info() {
                hyp.goto_locations.definition = self
                    .resolve_single_location(
                        text_document,
                        position,
                        GoToKind::Definition,
                        info.clone(),
                    )
                    .await;
                hyp.goto_locations.type_def = self
                    .resolve_single_location(text_document, position, GoToKind::Type, info)
                    .await;
            }
        }
    }

    async fn resolve_single_location(
        &self,
        text_document: &TextDocumentIdentifier,
        position: Position,
        kind: GoToKind,
        info: serde_json::Value,
    ) -> Option<GotoLocation> {
        let loc = self
            .get_goto_location(text_document, position, kind, info)
            .await
            .ok()??;
        Some(GotoLocation {
            uri: loc.target_uri,
            position: loc.target_selection_range.start,
        })
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

    fn parse_definition_response(response: &serde_json::Value) -> Option<LocationLink> {
        if response.is_null() {
            return None;
        }

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

        if let Ok(locs) = serde_json::from_value::<Vec<Location>>(response.clone()) {
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

        if let Ok(loc) = serde_json::from_value::<Location>(response.clone()) {
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

    /// Fetch combined goals (tactic + term) with pre-resolved goto locations.
    ///
    /// This is the high-level entry point for fetching goals at a position.
    /// It combines tactic goals and term goals, and pre-resolves goto locations
    /// for all hypotheses and targets.
    pub async fn fetch_combined_goals(
        &self,
        text_document: &TextDocumentIdentifier,
        position: Position,
    ) -> Result<Vec<Goal>, LspError> {
        let (tactic_result, term_result) = tokio::join!(
            self.get_goals(text_document, position),
            self.get_term_goal(text_document, position)
        );

        let term_goal = term_result.ok().flatten();
        let tactic_goals = match tactic_result {
            Ok(goals) => goals,
            Err(e) if term_goal.is_none() => return Err(e),
            Err(_) => vec![],
        };

        let mut goals: Vec<Goal> = term_goal.into_iter().chain(tactic_goals).collect();

        // Pre-resolve goto locations while RPC session is active
        for goal in &mut goals {
            self.resolve_goto_locations(text_document, position, goal)
                .await;
        }

        Ok(goals)
    }

    /// Fetch the proof DAG at a position using the LeanDag RPC method.
    ///
    /// This calls `LeanDag.getProofDag` which returns a complete proof DAG
    /// with parent/child relationships already computed server-side.
    pub async fn get_proof_dag(
        &self,
        text_document: &TextDocumentIdentifier,
        position: Position,
        mode: &str,
    ) -> Result<Option<ProofDag>, LspError> {
        let uri = &text_document.uri;
        let params = GetProofDagParams {
            text_document: text_document.clone(),
            position,
            mode: mode.to_string(),
        };

        let response = self
            .rpc_call_with_retry(uri, text_document, position, GET_PROOF_DAG, params)
            .await;

        match response {
            Ok(value) => {
                if value.is_null() {
                    tracing::debug!("LeanDag.getProofDag returned null");
                    return Ok(None);
                }
                let result: GetProofDagResult = serde_json::from_value(value)
                    .map_err(|e| LspError::ParseError(e.to_string()))?;
                tracing::debug!(
                    "LeanDag.getProofDag returned {} nodes",
                    result.proof_dag.nodes.len()
                );
                Ok(Some(result.proof_dag))
            }
            Err(LspError::RpcError { message, .. }) if message.contains("unknown method") => {
                // LeanDag not available - likely the project doesn't have the dependency
                tracing::debug!("LeanDag.getProofDag not available: {message}");
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
}
