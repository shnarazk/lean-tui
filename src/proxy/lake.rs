//! Lake serve child process management for the editor-facing LSP server.

use std::{
    env,
    fs::{self, File},
    path::PathBuf,
    process::Stdio,
};

use tokio::process::{ChildStdin, ChildStdout, Command};

use crate::error::{Error, LspError, Result};

/// Lean pretty-printer options for the server.
const LEAN_PP_OPTIONS: &[&str] = &[
    "pp.showLetValues=true", // Show full let-binding values (not ⋯)
];

/// Get the lake serve log file path, creating the directory if needed.
fn get_lake_serve_log_file() -> Option<File> {
    let home = env::var("HOME").ok()?;
    let log_dir = PathBuf::from(home).join(".cache/lean-tui");
    fs::create_dir_all(&log_dir).ok()?;

    let log_path = log_dir.join("lake-serve-editor.log");
    tracing::info!("lake serve log: {}", log_path.display());

    File::create(&log_path).ok()
}

/// Spawn lake serve for the editor-facing LSP connection.
///
/// This server handles standard LSP requests from the editor (hover, diagnostics, etc.).
/// The RPC client for proof DAGs is separate and spawns its own server.
pub fn spawn_lake_serve() -> Result<(ChildStdin, ChildStdout)> {
    // Log working directory for debugging
    if let Ok(cwd) = env::current_dir() {
        tracing::info!(
            "Spawning lake serve from working directory: {}",
            cwd.display()
        );
    }

    let mut cmd = Command::new("lake");
    cmd.arg("serve").arg("--");
    for opt in LEAN_PP_OPTIONS {
        cmd.args(["-D", opt]);
    }

    // Clear potentially conflicting environment
    cmd.env_remove("LEAN_PATH");
    cmd.env_remove("LEAN_SYSROOT");

    // Redirect stderr to log file for debugging
    let stderr = match get_lake_serve_log_file() {
        Some(file) => Stdio::from(file),
        None => Stdio::inherit(),
    };

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()?;

    let stdin = child.stdin.take().ok_or_else(|| {
        Error::Lsp(LspError::RpcError {
            code: None,
            message: "Failed to capture lake serve stdin".to_string(),
        })
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        Error::Lsp(LspError::RpcError {
            code: None,
            message: "Failed to capture lake serve stdout".to_string(),
        })
    })?;

    Ok((stdin, stdout))
}
