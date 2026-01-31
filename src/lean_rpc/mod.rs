mod base;
mod client;
mod dag;
mod lean_dag;
mod lean_server;

use async_lsp::lsp_types::{Position, Url};
pub use client::RpcClient;
pub use dag::{GoalInfo, HypothesisInfo, NodeId, ProofDag, ProofDagNode, ProofState};
use serde::{Deserialize, Serialize};

/// Pre-resolved `goto` location for navigation without RPC calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GotoLocation {
    pub uri: Url,
    pub position: Position,
}

/// Pre-resolved `goto` locations for multiple navigation kinds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoLocations {
    /// Location of the definition (for "go to definition").
    pub definition: Option<GotoLocation>,
    /// Location of the type definition (for "go to type definition").
    pub type_def: Option<GotoLocation>,
}

/// Diff status for goal state comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffTag {
    /// Sub-expression was modified (in "before" view)
    WasChanged,
    /// Sub-expression will be modified (in "after" view)
    WillChange,
    /// Sub-expression was deleted (in "before" view)
    WasDeleted,
    /// Sub-expression will be deleted (in "after" view)
    WillDelete,
    /// Sub-expression was inserted (in "before" view)
    WasInserted,
    /// Sub-expression will be inserted (in "after" view)
    WillInsert,
}

/// Sub-expression semantic information from Lean's `SubexprInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubexprInfo {
    /// Diff status for this sub expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_status: Option<DiffTag>,
}

/// Tagged text preserving Lean's `CodeWithInfos` structure with typed info.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[allow(clippy::use_self)] // Self in enum variants doesn't work with serde
pub enum TaggedText {
    /// Plain text leaf node.
    Text { text: String },

    /// Tagged span with semantic info wrapping nested content.
    Tag {
        info: SubexprInfo,
        content: Box<TaggedText>,
    },

    /// Concatenation of multiple items.
    Append { items: Vec<TaggedText> },
}

impl Default for TaggedText {
    fn default() -> Self {
        Self::Text {
            text: String::new(),
        }
    }
}

impl TaggedText {
    /// Extract plain text from the tagged structure (flattens all tags).
    pub fn to_plain_text(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::Tag { content, .. } => content.to_plain_text(),
            Self::Append { items } => items.iter().map(Self::to_plain_text).collect(),
        }
    }

    /// Check if any sub expression has a diff status set.
    pub fn has_any_diff(&self) -> bool {
        match self {
            Self::Text { .. } => false,
            Self::Tag { info, content } => info.diff_status.is_some() || content.has_any_diff(),
            Self::Append { items } => items.iter().any(Self::has_any_diff),
        }
    }
}

/// Response from `$/lean/rpc/connect`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcConnectResponse {
    pub session_id: String,
}

pub const RPC_CONNECT: &str = "$/lean/rpc/connect";
pub const RPC_CALL: &str = "$/lean/rpc/call";
pub const GET_PROOF_DAG: &str = "LeanDag.getProofDag";
