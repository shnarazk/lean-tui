//! Dedicated LSP client for lean-dag server.
//!
//! This client spawns its own lean-dag process and manages the full LSP lifecycle,
//! ensuring proper synchronization between document opening and RPC calls.

use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    ops::ControlFlow,
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
};

use async_lsp::{
    lsp_types::{
        notification::{DidChangeTextDocument, DidOpenTextDocument, Initialized},
        request::{Initialize, Request},
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams,
        InitializedParams, Position, TextDocumentIdentifier, Url,
    },
    AnyEvent, AnyNotification, AnyRequest, LspService, MainLoop, ResponseError, ServerSocket,
};
use serde::Serialize;
use serde_json::json;
use tokio::{
    process::Command,
    sync::{mpsc, Mutex, RwLock},
};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tower_service::Service;

use super::{dag::ProofDag, RpcConnectResponse, GET_PROOF_DAG, RPC_CALL, RPC_CONNECT};
use crate::error::LspError;

/// Lean pretty-printer options for the server.
const LEAN_PP_OPTIONS: &[&str] = &["pp.showLetValues=true"];

/// Document state tracked by the client.
struct DocumentState {
    version: u32,
    #[allow(dead_code)]
    content: String,
}

/// A simple service that receives notifications from the server.
struct LeanDagService {
    /// Sender to notify when files become ready.
    #[allow(dead_code)]
    ready_tx: mpsc::Sender<String>,
}

impl tower_service::Service<AnyRequest> for LeanDagService {
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

impl LspService for LeanDagService {
    fn notify(&mut self, notif: AnyNotification) -> ControlFlow<async_lsp::Result<()>> {
        if notif.method == "textDocument/publishDiagnostics" {
            if let Ok(params) =
                serde_json::from_value::<async_lsp::lsp_types::PublishDiagnosticsParams>(
                    notif.params,
                )
            {
                tracing::debug!("[LeanDag] publishDiagnostics for {}", params.uri);
            }
        }
        ControlFlow::Continue(())
    }

    fn emit(&mut self, _event: AnyEvent) -> ControlFlow<async_lsp::Result<()>> {
        ControlFlow::Continue(())
    }
}

#[derive(Serialize)]
struct RpcConnectParams {
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RpcCallParams<P> {
    text_document: TextDocumentIdentifier,
    position: Position,
    #[serde(serialize_with = "serialize_session_id")]
    session_id: u64,
    method: &'static str,
    params: P,
}

fn serialize_session_id<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&value.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetProofDagParams {
    text_document: TextDocumentIdentifier,
    position: Position,
    mode: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetProofDagResult {
    proof_dag: ProofDag,
    #[allow(dead_code)]
    version: u32,
}

#[derive(Serialize)]
struct WaitForDiagnosticsParams {
    uri: String,
    version: u32,
}

/// Dedicated LSP client for lean-dag.
///
/// Spawns its own lean-dag process and manages the full LSP lifecycle.
pub struct LeanDagClient {
    socket: ServerSocket,
    documents: RwLock<HashMap<String, DocumentState>>,
    sessions: Mutex<HashMap<String, u64>>,
    next_id: AtomicI64,
}

impl LeanDagClient {
    /// Create a new lean-dag client, spawning the server process.
    pub async fn new() -> Result<Arc<Self>, LspError> {
        let server_path = find_lean_dag_server()?;

        tracing::info!("[LeanDag] Starting server: {}", server_path.display());

        let (stdin, stdout) = spawn_lean_dag_server(&server_path)?;

        // Channel to receive ready notifications (for future use)
        let (ready_tx, _ready_rx) = mpsc::channel(32);

        // Create main loop with our service
        let (mainloop, socket) = MainLoop::new_client(|_| LeanDagService { ready_tx });

        // Run the mainloop in a background task
        tokio::spawn(async move {
            if let Err(e) = mainloop
                .run_buffered(stdout.compat(), stdin.compat_write())
                .await
            {
                tracing::error!("[LeanDag] MainLoop error: {:?}", e);
            }
        });

        let client = Arc::new(Self {
            socket,
            documents: RwLock::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
        });

        // Initialize the LSP connection
        client.initialize().await?;

        Ok(client)
    }

    /// Initialize the LSP connection.
    async fn initialize(&self) -> Result<(), LspError> {
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

        tracing::info!("[LeanDag] Sending initialize request");

        let id = self.next_request_id();
        let request_json =
            json!({ "id": id, "method": Initialize::METHOD, "params": params });
        let request: AnyRequest = serde_json::from_value(request_json)
            .map_err(|e| LspError::InvalidRequest(format!("Failed to build initialize request: {e}")))?;

        self.socket
            .clone()
            .call(request)
            .await
            .map_err(|e| LspError::RpcError {
                code: Some(e.code.0),
                message: format!(
                    "lean-dag initialization failed: {}. Check ~/.cache/lean-tui/lean-dag-client.log for details",
                    e.message
                ),
            })?;

        self.socket
            .notify::<Initialized>(InitializedParams {})
            .map_err(|e| LspError::RpcError {
                code: None,
                message: format!("Failed to complete initialization handshake: {e:?}"),
            })?;

        tracing::info!("[LeanDag] Server initialized");
        Ok(())
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

    /// Open a document in the lean-dag server.
    pub async fn did_open(&self, params: DidOpenTextDocumentParams) -> Result<(), LspError> {
        let uri = params.text_document.uri.to_string();
        let version = params.text_document.version as u32;
        let content = params.text_document.text.clone();

        tracing::info!("[LeanDag] didOpen {} v{}", uri, version);

        self.socket
            .notify::<DidOpenTextDocument>(params)
            .map_err(|e| LspError::RpcError {
                code: None,
                message: format!("Lost connection to lean-dag while opening document: {e:?}"),
            })?;

        self.documents
            .write()
            .await
            .insert(uri, DocumentState { version, content });

        Ok(())
    }

    /// Update a document in the lean-dag server.
    pub async fn did_change(&self, params: DidChangeTextDocumentParams) -> Result<(), LspError> {
        let uri = params.text_document.uri.to_string();
        let version = params.text_document.version as u32;

        tracing::info!("[LeanDag] didChange {} v{}", uri, version);

        self.socket
            .notify::<DidChangeTextDocument>(params)
            .map_err(|e| LspError::RpcError {
                code: None,
                message: format!("Lost connection to lean-dag while updating document: {e:?}"),
            })?;

        if let Some(doc) = self.documents.write().await.get_mut(&uri) {
            doc.version = version;
        }

        self.sessions.lock().await.remove(&uri);

        Ok(())
    }

    /// Wait for diagnostics to complete for a document.
    async fn wait_for_diagnostics(&self, uri: &Url, version: u32) -> Result<(), LspError> {
        let params = WaitForDiagnosticsParams {
            uri: uri.to_string(),
            version,
        };

        tracing::info!("[LeanDag] waitForDiagnostics {} v{}", uri, version);
        let _ = self
            .request("textDocument/waitForDiagnostics", params)
            .await?;
        tracing::info!("[LeanDag] File ready: {} v{}", uri, version);

        Ok(())
    }

    /// Get or create an RPC session for a document.
    async fn get_session(&self, uri: &Url) -> Result<u64, LspError> {
        let existing = self.sessions.lock().await.get(uri.as_str()).copied();
        if let Some(id) = existing {
            return Ok(id);
        }

        let params = RpcConnectParams {
            uri: uri.to_string(),
        };
        let response = self.request(RPC_CONNECT, params).await?;
        let resp: RpcConnectResponse = serde_json::from_value(response).map_err(|e| {
            LspError::ParseError(format!("Invalid RPC session response: {e}"))
        })?;

        let session_id = resp.session_id;
        tracing::info!("[LeanDag] RPC session for {}: {}", uri, session_id);

        self.sessions
            .lock()
            .await
            .insert(uri.to_string(), session_id);
        Ok(session_id)
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

        // Get RPC session
        let session_id = self.get_session(uri).await?;

        // Make RPC call
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

/// Find the lean-dag server binary.
///
/// Search order:
/// 1. LEAN_DAG_SERVER environment variable (development override)
/// 2. Git-imported LeanDag package at .lake/packages/LeanDag/.lake/build/bin/lean-dag
fn find_lean_dag_server() -> Result<PathBuf, LspError> {
    let mut searched_paths = Vec::new();

    // 1. Environment variable override (for development)
    if let Ok(path) = env::var("LEAN_DAG_SERVER") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
        searched_paths.push(format!("$LEAN_DAG_SERVER={path}"));
    }

    // 2. Git-imported LeanDag package (uses project's toolchain)
    let cwd = env::current_dir().map_err(|e| LspError::RpcError {
        code: None,
        message: format!("Cannot determine current directory: {e}"),
    })?;

    let package_binary = cwd.join(".lake/packages/LeanDag/.lake/build/bin/lean-dag");
    searched_paths.push(package_binary.display().to_string());

    if package_binary.exists() {
        return Ok(package_binary);
    }

    Err(LspError::LeanDagNotFound { searched_paths })
}

/// Get the lean-dag log file path.
fn get_lean_dag_log_file() -> Option<File> {
    let home = env::var("HOME").ok()?;
    let log_dir = PathBuf::from(home).join(".cache/lean-tui");
    fs::create_dir_all(&log_dir).ok()?;
    let log_path = log_dir.join("lean-dag-client.log");
    File::create(&log_path).ok()
}

/// Spawn the lean-dag server process.
fn spawn_lean_dag_server(
    server_path: &PathBuf,
) -> Result<(tokio::process::ChildStdin, tokio::process::ChildStdout), LspError> {
    let server_str = server_path.display().to_string();
    let pp_opts: String = LEAN_PP_OPTIONS
        .iter()
        .map(|opt| format!("-D {opt}"))
        .collect::<Vec<_>>()
        .join(" ");

    let shell_cmd = format!("LEAN_WORKER_PATH={server_str} exec {server_str} -- {pp_opts}");

    let mut cmd = Command::new("lake");
    cmd.args(["env", "sh", "-c", &shell_cmd]);
    cmd.env_remove("LEAN_PATH");
    cmd.env_remove("LEAN_SYSROOT");

    let stderr = match get_lean_dag_log_file() {
        Some(file) => Stdio::from(file),
        None => Stdio::inherit(),
    };

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .map_err(|e| LspError::LeanDagSpawnFailed {
            path: server_str.clone(),
            reason: e.to_string(),
        })?;

    let stdin = child.stdin.take().ok_or_else(|| LspError::LeanDagSpawnFailed {
        path: server_str.clone(),
        reason: "Failed to capture stdin pipe".to_string(),
    })?;
    let stdout = child.stdout.take().ok_or_else(|| LspError::LeanDagSpawnFailed {
        path: server_str,
        reason: "Failed to capture stdout pipe".to_string(),
    })?;

    Ok((stdin, stdout))
}
