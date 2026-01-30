//! Lake serve child process management.

use std::{env, path::PathBuf, process::Stdio};

use tokio::process::{ChildStdin, ChildStdout, Command};

use crate::error::{Error, LspError, Result};

/// Lean pretty-printer options for the server.
const LEAN_PP_OPTIONS: &[&str] = &[
    "pp.showLetValues=true", // Show full let-binding values (not ⋯)
];

/// Find the lean-dag server binary.
///
/// Checks in order:
/// 1. LEAN_DAG_SERVER environment variable
/// 2. ../lean-dag/.lake/build/bin/lean-dag (sibling directory)
fn find_lean_dag_server() -> Option<PathBuf> {
    // Check environment variable first
    if let Ok(path) = env::var("LEAN_DAG_SERVER") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // Check sibling directory (development setup)
    let cwd = env::current_dir().ok()?;
    let sibling = cwd.parent()?.join("lean-dag/.lake/build/bin/lean-dag");
    if sibling.exists() {
        return Some(sibling);
    }

    None
}

/// Spawn Lean server with LeanDag RPC methods.
///
/// Uses `lake env` to get the project environment, then runs the lean-dag
/// server binary with that environment. This makes the RPC method available
/// without requiring users to import LeanDag in their files.
///
/// Falls back to `lake serve` if lean-dag server is not found.
pub fn spawn_lake_serve() -> Result<(ChildStdin, ChildStdout)> {
    match find_lean_dag_server() {
        Some(server_path) => {
            tracing::info!("Using lean-dag server: {}", server_path.display());
            spawn_lean_dag_server(&server_path)
        }
        None => {
            tracing::warn!("lean-dag server not found, falling back to lake serve (RPC will not work)");
            spawn_standard_lake_serve()
        }
    }
}

/// Spawn the lean-dag server with the current project's lake environment.
///
/// Uses the same shell-based approach as lean-dag's test suite to ensure
/// LEAN_WORKER_PATH is properly set for worker processes.
fn spawn_lean_dag_server(server_path: &PathBuf) -> Result<(ChildStdin, ChildStdout)> {
    let server_str = server_path.display();
    let pp_opts: String = LEAN_PP_OPTIONS
        .iter()
        .map(|opt| format!("-D {opt}"))
        .collect::<Vec<_>>()
        .join(" ");

    // Match lean-dag test suite: use shell to set LEAN_WORKER_PATH inside lake env
    // This ensures workers use the lean-dag binary with RPC methods registered
    let shell_cmd = format!("LEAN_WORKER_PATH={server_str} exec {server_str} -- {pp_opts}");

    let mut cmd = Command::new("lake");
    cmd.args(["env", "sh", "-c", &shell_cmd]);

    // Clear potentially conflicting environment
    cmd.env_remove("LEAN_PATH");
    cmd.env_remove("LEAN_SYSROOT");

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let stdin = child.stdin.take().ok_or_else(|| {
        Error::Lsp(LspError::RpcError {
            code: None,
            message: "Failed to capture lean-dag server stdin".to_string(),
        })
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        Error::Lsp(LspError::RpcError {
            code: None,
            message: "Failed to capture lean-dag server stdout".to_string(),
        })
    })?;

    Ok((stdin, stdout))
}

/// Spawn standard lake serve (fallback).
fn spawn_standard_lake_serve() -> Result<(ChildStdin, ChildStdout)> {
    let mut cmd = Command::new("lake");
    cmd.arg("serve").arg("--");
    for opt in LEAN_PP_OPTIONS {
        cmd.args(["-D", opt]);
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
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
