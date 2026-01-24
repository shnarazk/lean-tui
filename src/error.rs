use std::{fmt, io};

/// Structured LSP/RPC error type for better error handling semantics.
#[derive(Debug)]
pub enum LspError {
    /// RPC session expired and needs reconnection.
    SessionExpired,
    /// Invalid request format or parameters.
    InvalidRequest(String),
    /// Failed to parse response.
    ParseError(String),
    /// RPC error with optional error code.
    RpcError { code: Option<i32>, message: String },
}

impl fmt::Display for LspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionExpired => write!(f, "RPC session expired"),
            Self::InvalidRequest(msg) => write!(f, "Invalid request: {msg}"),
            Self::ParseError(msg) => write!(f, "Parse error: {msg}"),
            Self::RpcError {
                code: Some(c),
                message,
            } => write!(f, "RPC error {c}: {message}"),
            Self::RpcError {
                code: None,
                message,
            } => write!(f, "RPC error: {message}"),
        }
    }
}

impl std::error::Error for LspError {}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Json(serde_json::Error),
    Lsp(LspError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Lsp(e) => write!(f, "LSP error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Lsp(e) => Some(e),
        }
    }
}

impl From<LspError> for Error {
    fn from(e: LspError) -> Self {
        Self::Lsp(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
