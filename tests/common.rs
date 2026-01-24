#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

pub struct LspTestHarness {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    stderr: Option<ChildStderr>,
}

impl LspTestHarness {
    pub fn spawn() -> Self {
        // Ensure release binary is built
        let status = Command::new("cargo")
            .args(["build", "--release", "--quiet"])
            .status()
            .expect("Failed to build");
        assert!(status.success(), "Build failed");

        let mut child = Command::new("./target/release/lean-tui")
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn lean-tui");

        let stdin = child.stdin.take();
        let stdout = child.stdout.take().map(BufReader::new);
        let stderr = child.stderr.take();

        Self {
            child: Some(child),
            stdin,
            stdout,
            stderr,
        }
    }

    pub fn send(&mut self, content: &str) -> std::io::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stdin closed"))?;
        let msg = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
        stdin.write_all(msg.as_bytes())?;
        stdin.flush()
    }

    pub fn read_response(&mut self) -> Option<String> {
        let stdout = self.stdout.as_mut()?;

        let mut header = String::new();
        stdout.read_line(&mut header).ok()?;

        if !header.starts_with("Content-Length:") {
            return None;
        }

        let len: usize = header
            .trim()
            .strip_prefix("Content-Length:")?
            .trim()
            .parse()
            .ok()?;

        // Read empty line
        let mut empty = String::new();
        stdout.read_line(&mut empty).ok()?;

        // Read body
        let mut body = vec![0u8; len];
        std::io::Read::read_exact(stdout, &mut body).ok()?;

        String::from_utf8(body).ok()
    }

    pub fn initialize(&mut self) -> Option<String> {
        let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{},"rootUri":"file:///home/wvhulle/Code/lean-tui","processId":1234}}"#;
        self.send(init).ok()?;
        std::thread::sleep(Duration::from_secs(3));
        self.read_response()
    }

    pub fn initialized(&mut self) {
        let msg = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
        let _ = self.send(msg);
        std::thread::sleep(Duration::from_millis(500));
    }

    pub fn shutdown(&mut self) {
        let shutdown = r#"{"jsonrpc":"2.0","id":99,"method":"shutdown","params":null}"#;
        let _ = self.send(shutdown);
        std::thread::sleep(Duration::from_millis(200));

        let exit = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;
        let _ = self.send(exit);
    }

    /// Close stdin and collect stderr output
    pub fn collect_stderr(&mut self) -> String {
        // Close stdin to signal EOF
        self.stdin.take();

        // Read all stderr lines
        let mut output = String::new();
        if let Some(stderr) = self.stderr.take() {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        output.push_str(&l);
                        output.push('\n');
                    }
                    Err(_) => break,
                }
            }
        }

        output
    }
}

impl Drop for LspTestHarness {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
        }
    }
}
