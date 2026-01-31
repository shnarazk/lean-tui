//! LSP client for library mode using standard `lake serve`.
//!
//! Spawns a `lake serve` process. Users must `import LeanDag` in their
//! Lean files for the RPC methods to be available.

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

/// LSP client for library mode using standard `lake serve`.
///
/// Users must add `import LeanDag` to their Lean files for the
/// `LeanDag.getProofDag` RPC method to be available.
pub struct LeanServerClient {
    base: Arc<BaseLspClient>,
}

impl LeanServerClient {
    /// Create a new lean server client, spawning `lake serve`.
    pub async fn new() -> Result<Arc<Self>, LspError> {
        tracing::info!("[LeanServer] Starting lake serve");

        let (stdin, stdout) = spawn_lake_serve()?;

        // Create main loop with our service
        let (mainloop, socket) = MainLoop::new_client(|_| LeanService::new("LeanServer"));

        // Run the mainloop in a background task
        tokio::spawn(async move {
            if let Err(e) = mainloop
                .run_buffered(stdout.compat(), stdin.compat_write())
                .await
            {
                tracing::error!("[LeanServer] MainLoop error: {:?}", e);
            }
        });

        let base = Arc::new(BaseLspClient::new("LeanServer", socket));

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

/// Get the lake serve log file path.
fn get_lake_serve_log_file() -> Option<File> {
    let home = env::var("HOME").ok()?;
    let log_dir = PathBuf::from(home).join(".cache/lean-tui");
    fs::create_dir_all(&log_dir).ok()?;
    let log_path = log_dir.join("lake-serve.log");
    File::create(&log_path).ok()
}

/// Spawn the lake serve process.
fn spawn_lake_serve() -> Result<(ChildStdin, ChildStdout), LspError>
{
    let mut cmd = Command::new("lake");
    cmd.arg("serve").arg("--");

    for opt in LEAN_PP_OPTIONS {
        cmd.args(["-D", opt]);
    }

    cmd.env_remove("LEAN_PATH");
    cmd.env_remove("LEAN_SYSROOT");

    let stderr = get_lake_serve_log_file().map_or_else(Stdio::inherit, Stdio::from);

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .map_err(|e| LspError::RpcError {
            code: None,
            message: format!("Failed to spawn lake serve: {e}"),
        })?;

    let stdin = child.stdin.take().ok_or_else(|| LspError::RpcError {
        code: None,
        message: "Failed to capture lake serve stdin".to_string(),
    })?;
    let stdout = child.stdout.take().ok_or_else(|| LspError::RpcError {
        code: None,
        message: "Failed to capture lake serve stdout".to_string(),
    })?;

    Ok((stdin, stdout))
}
