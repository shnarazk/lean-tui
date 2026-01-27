//! Lake serve child process management.

use std::process::Stdio;

use tokio::process::{ChildStdin, ChildStdout, Command};

use crate::error::{Error, LspError, Result};

/// Lean pretty-printer options for the server.
const LEAN_PP_OPTIONS: &[&str] = &[
    "pp.showLetValues=true", // Show full let-binding values (not ⋯)
];

/// Spawn lake serve child process with configured Lean options.
pub fn spawn_lake_serve() -> Result<(ChildStdin, ChildStdout)> {
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
