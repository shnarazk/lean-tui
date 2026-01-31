//! Shared LSP client infrastructure for both library and standalone modes.

use std::{
    collections::HashMap,
    env,
    ops::ControlFlow,
    sync::atomic::{AtomicI64, Ordering},
};

use async_lsp::{
    lsp_types::{
        notification::{DidChangeTextDocument, DidOpenTextDocument, Initialized},
        request::{Initialize, Request},
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams,
        InitializedParams, Position, TextDocumentIdentifier, Url,
    },
    AnyEvent, AnyNotification, AnyRequest, LspService, ResponseError, ServerSocket,
};
use serde::Serialize;
use serde_json::json;
use tokio::sync::{Mutex, RwLock};
use tower_service::Service;

use super::{ProofDag, RpcConnectResponse, GET_PROOF_DAG, RPC_CALL, RPC_CONNECT};
use crate::error::LspError;

/// Lean pretty-printer options for the server.
pub const LEAN_PP_OPTIONS: &[&str] = &["pp.showLetValues=true"];

/// Lean LSP error code for outdated RPC session.
const RPC_SESSION_OUTDATED: i32 = -32900;

/// Document state tracked by the client.
pub struct DocumentState {
    pub version: u32,
}

/// A simple service that receives notifications from the server.
pub struct LeanService {
    name: &'static str,
}

impl LeanService {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl tower_service::Service<AnyRequest> for LeanService {
    type Response = serde_json::Value;
    type Error = ResponseError;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: AnyRequest) -> Self::Future {
        Box::pin(async { Ok(serde_json::Value::Null) })
    }
}

impl LspService for LeanService {
    fn notify(&mut self, notif: AnyNotification) -> ControlFlow<async_lsp::Result<()>> {
        if notif.method == "textDocument/publishDiagnostics" {
            if let Ok(params) = serde_json::from_value::<
                async_lsp::lsp_types::PublishDiagnosticsParams,
            >(notif.params)
            {
                tracing::debug!("[{}] publishDiagnostics for {}", self.name, params.uri);
            }
        }
        ControlFlow::Continue(())
    }

    fn emit(&mut self, _event: AnyEvent) -> ControlFlow<async_lsp::Result<()>> {
        ControlFlow::Continue(())
    }
}

#[derive(Serialize)]
pub struct RpcConnectParams {
    pub uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcCallParams<P> {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    #[serde(serialize_with = "serialize_session_id")]
    pub session_id: u64,
    pub method: &'static str,
    pub params: P,
}

fn serialize_session_id<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&value.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProofDagParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub mode: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProofDagResult {
    pub proof_dag: ProofDag,
}

#[derive(Serialize)]
pub struct WaitForDiagnosticsParams {
    pub uri: String,
    pub version: u32,
}

/// Base LSP client with shared functionality.
///
/// Both `LeanServerClient` and `LeanDagClient` wrap this with different
/// server spawning logic.
pub struct BaseLspClient {
    name: &'static str,
    socket: ServerSocket,
    documents: RwLock<HashMap<String, DocumentState>>,
    sessions: Mutex<HashMap<String, u64>>,
    next_id: AtomicI64,
}

impl BaseLspClient {
    /// Create a new base client with the given socket.
    pub fn new(name: &'static str, socket: ServerSocket) -> Self {
        Self {
            name,
            socket,
            documents: RwLock::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
        }
    }

    /// Initialize the LSP connection.
    pub async fn initialize(&self) -> Result<(), LspError> {
        let cwd = env::current_dir().map_err(|e| LspError::RpcError {
            code: None,
            message: format!("Cannot determine working directory: {e}"),
        })?;

        let root_uri = Url::from_file_path(&cwd).map_err(|_| LspError::RpcError {
            code: None,
            message: format!("Invalid project path: {}", cwd.display()),
        })?;

        #[allow(deprecated)]
        let params = InitializeParams {
            root_uri: Some(root_uri),
            capabilities: Default::default(),
            ..Default::default()
        };

        tracing::debug!("[{}] Sending initialize request", self.name);

        let id = self.next_request_id();
        let request_json = json!({ "id": id, "method": Initialize::METHOD, "params": params });
        let request: AnyRequest = serde_json::from_value(request_json).map_err(|e| {
            LspError::InvalidRequest(format!("Failed to build initialize request: {e}"))
        })?;

        self.socket
            .clone()
            .call(request)
            .await
            .map_err(|e| LspError::RpcError {
                code: Some(e.code.0),
                message: format!(
                    "{} initialization failed: {}. Check logs for details",
                    self.name, e.message
                ),
            })?;

        self.socket
            .notify::<Initialized>(InitializedParams {})
            .map_err(|e| LspError::RpcError {
                code: None,
                message: format!("Failed to complete initialization handshake: {e:?}"),
            })?;

        tracing::info!("[{}] Server initialized", self.name);
        Ok(())
    }

    fn next_request_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn request(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<serde_json::Value, LspError> {
        let id = self.next_request_id();
        let request_json = json!({ "id": id, "method": method, "params": params });
        let request: AnyRequest = serde_json::from_value(request_json)
            .map_err(|e| LspError::InvalidRequest(format!("Failed to serialize {method}: {e}")))?;

        self.socket
            .clone()
            .call(request)
            .await
            .map_err(|e| LspError::RpcError {
                code: Some(e.code.0),
                message: format!("{method} failed: {}", e.message),
            })
    }

    /// Open a document in the server.
    pub async fn did_open(&self, params: DidOpenTextDocumentParams) -> Result<(), LspError> {
        let uri = params.text_document.uri.to_string();
        let version = params.text_document.version as u32;

        tracing::debug!("[{}] didOpen {} v{}", self.name, uri, version);

        self.socket
            .notify::<DidOpenTextDocument>(params)
            .map_err(|e| LspError::RpcError {
                code: None,
                message: format!(
                    "Lost connection to {} while opening document: {e:?}",
                    self.name
                ),
            })?;

        self.documents
            .write()
            .await
            .insert(uri, DocumentState { version });

        Ok(())
    }

    /// Update a document in the server.
    pub async fn did_change(&self, params: DidChangeTextDocumentParams) -> Result<(), LspError> {
        let uri = params.text_document.uri.to_string();
        let version = params.text_document.version as u32;

        tracing::debug!("[{}] didChange {} v{}", self.name, uri, version);

        self.socket
            .notify::<DidChangeTextDocument>(params)
            .map_err(|e| LspError::RpcError {
                code: None,
                message: format!(
                    "Lost connection to {} while updating document: {e:?}",
                    self.name
                ),
            })?;

        if let Some(doc) = self.documents.write().await.get_mut(&uri) {
            doc.version = version;
        }

        self.sessions.lock().await.remove(&uri);

        Ok(())
    }

    /// Wait for diagnostics to complete for a document.
    pub async fn wait_for_diagnostics(&self, uri: &Url, version: u32) -> Result<(), LspError> {
        let params = WaitForDiagnosticsParams {
            uri: uri.to_string(),
            version,
        };

        tracing::debug!(
            "[{}] waitForDiagnostics {} v{}",
            self.name,
            uri,
            version
        );
        let _ = self
            .request("textDocument/waitForDiagnostics", params)
            .await?;
        tracing::debug!("[{}] File ready: {} v{}", self.name, uri, version);

        Ok(())
    }

    /// Get or create an RPC session for a document.
    pub async fn get_session(&self, uri: &Url) -> Result<u64, LspError> {
        let existing = self.sessions.lock().await.get(uri.as_str()).copied();
        if let Some(id) = existing {
            return Ok(id);
        }

        self.create_session(uri).await
    }

    /// Create a new RPC session for a document, replacing any existing one.
    pub async fn create_session(&self, uri: &Url) -> Result<u64, LspError> {
        let params = RpcConnectParams {
            uri: uri.to_string(),
        };
        let response = self.request(RPC_CONNECT, params).await?;
        let resp: RpcConnectResponse = serde_json::from_value(response)
            .map_err(|e| LspError::ParseError(format!("Invalid RPC session response: {e}")))?;

        let session_id = resp.session_id;
        tracing::debug!("[{}] RPC session for {}: {}", self.name, uri, session_id);

        self.sessions
            .lock()
            .await
            .insert(uri.to_string(), session_id);
        Ok(session_id)
    }

    /// Invalidate the RPC session for a document.
    pub async fn invalidate_session(&self, uri: &Url) {
        self.sessions.lock().await.remove(uri.as_str());
    }

    /// Get the proof DAG at a position.
    pub async fn get_proof_dag(
        &self,
        uri: &Url,
        position: Position,
        mode: &str,
    ) -> Result<Option<ProofDag>, LspError> {
        // Get document version
        let version = self
            .documents
            .read()
            .await
            .get(uri.as_str())
            .map(|d| d.version)
            .unwrap_or(1);

        // Wait for diagnostics first
        self.wait_for_diagnostics(uri, version).await?;

        // Try with existing session, retry once if session is outdated
        match self.try_get_proof_dag(uri, position, mode).await {
            Ok(result) => Ok(result),
            Err(LspError::RpcError {
                code: Some(code), ..
            }) if code == RPC_SESSION_OUTDATED => {
                tracing::debug!("[{}] Session outdated for {}, renewing", self.name, uri);
                self.invalidate_session(uri).await;
                self.try_get_proof_dag(uri, position, mode).await
            }
            Err(e) => Err(e),
        }
    }

    /// Internal: attempt to get proof DAG with current session.
    async fn try_get_proof_dag(
        &self,
        uri: &Url,
        position: Position,
        mode: &str,
    ) -> Result<Option<ProofDag>, LspError> {
        let session_id = self.get_session(uri).await?;

        let text_document = TextDocumentIdentifier { uri: uri.clone() };
        let inner_params = GetProofDagParams {
            text_document: text_document.clone(),
            position,
            mode: mode.to_string(),
        };

        let params = RpcCallParams {
            text_document,
            position,
            session_id,
            method: GET_PROOF_DAG,
            params: inner_params,
        };

        let response = self.request(RPC_CALL, params).await?;

        if response.is_null() {
            return Ok(None);
        }

        let result: GetProofDagResult = serde_json::from_value(response).map_err(|e| {
            LspError::ParseError(format!("Failed to parse proof DAG response: {e}"))
        })?;

        Ok(Some(result.proof_dag))
    }
}
