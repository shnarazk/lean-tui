//! LSP client for standalone mode using the lean-dag binary.
//!
//! Spawns its own lean-dag process which has the RPC methods built-in
//! via `builtin_initialize`. No `import LeanDag` required in user files.

use std::{
    env,
    fs::{self, File},
    path::PathBuf,
    process::Stdio,
    sync::Arc,
};

use async_lsp::{
    lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, Position, Url},
    MainLoop,
};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use super::{
    base::{BaseLspClient, LeanService, LEAN_PP_OPTIONS},
    ProofDag,
};
use crate::error::LspError;

/// LSP client for standalone mode using the lean-dag binary.
pub struct LeanDagClient {
    base: Arc<BaseLspClient>,
}

impl LeanDagClient {
    /// Create a new lean-dag client, spawning the server process.
    pub async fn new() -> Result<Arc<Self>, LspError> {
        let server_path = find_lean_dag_server()?;

        tracing::info!("[LeanDag] Starting server: {}", server_path.display());

        let (stdin, stdout) = spawn_lean_dag_server(&server_path)?;

        // Create main loop with our service
        let (mainloop, socket) = MainLoop::new_client(|_| LeanService::new("LeanDag"));

        // Run the mainloop in a background task
        tokio::spawn(async move {
            if let Err(e) = mainloop
                .run_buffered(stdout.compat(), stdin.compat_write())
                .await
            {
                tracing::error!("[LeanDag] MainLoop error: {:?}", e);
            }
        });

        let base = Arc::new(BaseLspClient::new("LeanDag", socket));

        // Initialize the LSP connection
        base.initialize().await?;

        Ok(Arc::new(Self { base }))
    }

    /// Open a document in the server.
    pub async fn did_open(&self, params: DidOpenTextDocumentParams) -> Result<(), LspError> {
        self.base.did_open(params).await
    }

    /// Update a document in the server.
    pub async fn did_change(&self, params: DidChangeTextDocumentParams) -> Result<(), LspError> {
        self.base.did_change(params).await
    }

    /// Get the proof DAG at a position.
    pub async fn get_proof_dag(
        &self,
        uri: &Url,
        position: Position,
        mode: &str,
    ) -> Result<Option<ProofDag>, LspError> {
        self.base.get_proof_dag(uri, position, mode).await
    }
}

/// Find the Lake project root by searching upward for `lakefile.lean`.
fn find_lake_root() -> Option<PathBuf> {
    let mut current = env::current_dir().ok()?;
    loop {
        if current.join("lakefile.lean").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Find the lean-dag server binary.
///
/// Search order:
/// 1. `LEAN_DAG_SERVER` environment variable (development override)
/// 2. Git-imported `LeanDag` package at
///    `.lake/packages/LeanDag/.lake/build/bin/lean-dag`
/// 3. Sibling directory `../lean-dag/.lake/build/bin/lean-dag`
fn find_lean_dag_server() -> Result<PathBuf, LspError> {
    let mut searched_paths = Vec::new();

    // 1. Environment variable override (for development)
    if let Ok(path) = env::var("LEAN_DAG_SERVER") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
        searched_paths.push(p);
    }

    // 2. Find Lake project root
    let project_root = find_lake_root();

    if let Some(ref root) = project_root {
        // Check for LeanDag as a Lake package
        let package_binary = root.join(".lake/packages/LeanDag/.lake/build/bin/lean-dag");
        searched_paths.push(package_binary.clone());
        if package_binary.exists() {
            return Ok(package_binary);
        }

        // Check sibling directory (development setup)
        if let Some(parent) = root.parent() {
            let sibling = parent.join("lean-dag/.lake/build/bin/lean-dag");
            searched_paths.push(sibling.clone());
            if sibling.exists() {
                return Ok(sibling);
            }
        }
    }

    Err(LspError::LeanDagNotFound {
        searched_paths,
        project_root,
    })
}

/// Get the lean-dag log file path.
fn get_lean_dag_log_file() -> Option<File> {
    let home = env::var("HOME").ok()?;
    let log_dir = PathBuf::from(home).join(".cache/lean-tui");
    fs::create_dir_all(&log_dir).ok()?;
    let log_path = log_dir.join("lean-dag.log");
    File::create(&log_path).ok()
}

/// Spawn the lean-dag server process.
///
/// lean-dag handles its own environment discovery internally by calling
/// `lake env printenv` at startup, so we can spawn it directly without
/// wrapping in `lake env`.
fn spawn_lean_dag_server(server_path: &PathBuf) -> Result<(ChildStdin, ChildStdout), LspError> {
    let server_str = server_path.display().to_string();

    // Build command-line arguments for pretty-printing options
    let mut args: Vec<String> = Vec::new();
    for opt in LEAN_PP_OPTIONS {
        args.push("-D".to_string());
        args.push(opt.to_string());
    }

    let mut cmd = Command::new(server_path);
    cmd.args(&args);

    // Set LEAN_WORKER_PATH so worker processes also use the lean-dag binary
    cmd.env("LEAN_WORKER_PATH", server_path);

    let stderr = get_lean_dag_log_file().map_or_else(Stdio::inherit, Stdio::from);

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .map_err(|e| LspError::LeanDagSpawnFailed {
            path: server_str.clone(),
            reason: e.to_string(),
        })?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| LspError::LeanDagSpawnFailed {
            path: server_str.clone(),
            reason: "Failed to capture stdin pipe".to_string(),
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| LspError::LeanDagSpawnFailed {
            path: server_str,
            reason: "Failed to capture stdout pipe".to_string(),
        })?;

    Ok((stdin, stdout))
}
