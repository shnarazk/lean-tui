//! Lean RPC protocol types and clients.
//!
//! This module provides two LSP clients for fetching proof DAGs:
//!
//! - [`LeanServerClient`] - Library mode: uses `lake serve`, requires `import LeanDag`
//! - [`LeanDagClient`] - Standalone mode: uses lean-dag binary, no import required
//!
//! Use [`RpcClient::new(standalone)`] to create the appropriate client.

mod base;
mod client;
mod dag;
mod lean_dag;
mod lean_server;

use async_lsp::lsp_types::{Position, Url};
pub use client::RpcClient;
pub use dag::{GoalInfo, HypothesisInfo, NodeId, ProofDag, ProofDagNode, ProofState};
use serde::{Deserialize, Serialize};

/// Pre-resolved goto location for navigation without RPC calls.
///
/// Stored alongside hypotheses and goals when they're fetched, so navigation
/// doesn't depend on RPC session validity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GotoLocation {
    pub uri: Url,
    pub position: Position,
}

/// Pre-resolved goto locations for multiple navigation kinds.
///
/// Holds both definition and type definition locations, resolved at goal fetch
/// time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoLocations {
    /// Location of the definition (for "go to definition").
    pub definition: Option<GotoLocation>,
    /// Location of the type definition (for "go to type definition").
    pub type_def: Option<GotoLocation>,
}

/// Diff status for goal state comparisons.
///
/// The Lean server marks subexpressions with these tags when comparing
/// goal states (e.g., before/after a tactic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffTag {
    /// Subexpression was modified (in "before" view)
    WasChanged,
    /// Subexpression will be modified (in "after" view)
    WillChange,
    /// Subexpression was deleted (in "before" view)
    WasDeleted,
    /// Subexpression will be deleted (in "after" view)
    WillDelete,
    /// Subexpression was inserted (in "before" view)
    WasInserted,
    /// Subexpression will be inserted (in "after" view)
    WillInsert,
}

/// Subexpression semantic information from Lean's `SubexprInfo`.
///
/// This preserves the full structure from Lean, enabling:
/// - Per-subexpression diff highlighting via `diff_status`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubexprInfo {
    /// Diff status for this subexpression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_status: Option<DiffTag>,
}

/// Tagged text preserving Lean's `CodeWithInfos` structure with typed info.
///
/// This type provides per-subexpression diff highlighting.
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

    /// Check if any subexpression has a diff status set.
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
    #[serde(deserialize_with = "deserialize_session_id")]
    pub session_id: u64,
}

/// Deserialize session ID from either a number or a string.
fn deserialize_session_id<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct SessionIdVisitor;

    impl<'de> Visitor<'de> for SessionIdVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a u64 or a string containing a u64")
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<u64, E> {
            Ok(value)
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<u64, E> {
            value.parse().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(SessionIdVisitor)
}

pub const RPC_CONNECT: &str = "$/lean/rpc/connect";
pub const RPC_CALL: &str = "$/lean/rpc/call";
pub const GET_PROOF_DAG: &str = "LeanDag.getProofDag";
