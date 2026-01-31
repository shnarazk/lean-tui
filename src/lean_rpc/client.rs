//! RPC client enum for proof DAG fetching.

use std::sync::Arc;

use async_lsp::lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, Position, Url};

use super::{lean_dag::LeanDagClient, lean_server::LeanServerClient, ProofDag};
use crate::error::LspError;

/// RPC client for fetching proof DAGs.
///
/// This enum wraps both client implementations:
/// - `LeanServer`: Library mode - uses `lake serve`, requires `import LeanDag`
/// - `LeanDag`: Standalone mode - uses lean-dag binary, no import required
#[derive(Clone)]
pub enum RpcClient {
    /// Library mode client using `lake serve`.
    LeanServer(Arc<LeanServerClient>),
    /// Standalone mode client using lean-dag binary.
    LeanDag(Arc<LeanDagClient>),
}

impl RpcClient {
    /// Create an RPC client based on the server mode.
    ///
    /// - `standalone = true`: Uses [`LeanDagClient`] (lean-dag binary)
    /// - `standalone = false`: Uses [`LeanServerClient`] (lake serve)
    pub async fn new(standalone: bool) -> Result<Self, LspError> {
        if standalone {
            let client = LeanDagClient::new().await?;
            Ok(Self::LeanDag(client))
        } else {
            let client = LeanServerClient::new().await?;
            Ok(Self::LeanServer(client))
        }
    }

    /// Open a document in the server.
    pub async fn did_open(&self, params: DidOpenTextDocumentParams) -> Result<(), LspError> {
        match self {
            Self::LeanServer(client) => client.did_open(params).await,
            Self::LeanDag(client) => client.did_open(params).await,
        }
    }

    /// Update a document in the server.
    pub async fn did_change(&self, params: DidChangeTextDocumentParams) -> Result<(), LspError> {
        match self {
            Self::LeanServer(client) => client.did_change(params).await,
            Self::LeanDag(client) => client.did_change(params).await,
        }
    }

    /// Get the proof DAG at a position.
    pub async fn get_proof_dag(
        &self,
        uri: &Url,
        position: Position,
        mode: &str,
    ) -> Result<Option<ProofDag>, LspError> {
        match self {
            Self::LeanServer(client) => client.get_proof_dag(uri, position, mode).await,
            Self::LeanDag(client) => client.get_proof_dag(uri, position, mode).await,
        }
    }
}
