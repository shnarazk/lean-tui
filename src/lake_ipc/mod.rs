//! Lean RPC protocol types and client for communication with lake serve.
//!
//! Lean uses custom RPC methods tunneled through LSP:
//! - `$/lean/rpc/connect` - establish session
//! - `$/lean/rpc/keepAlive` - maintain session
//! - `$/lean/rpc/call` - invoke Lean methods like `getInteractiveGoals`

mod rpc_client;

pub use rpc_client::RpcClient;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parameters for `$/lean/rpc/connect` request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConnectParams {
    pub uri: String,
}

/// Response from `$/lean/rpc/connect`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcConnectResponse {
    pub session_id: String,
}

/// Parameters for `$/lean/rpc/keepAlive` notification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcKeepAliveParams {
    pub uri: String,
    pub session_id: String,
}

/// Parameters for `$/lean/rpc/call` request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcCallParams {
    pub session_id: String,
    pub method: String,
    pub params: Value,
}

/// Parameters for `Lean.Widget.getInteractiveGoals` RPC method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetInteractiveGoalsParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Simplified goal representation for TUI display
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Goal {
    /// Hypotheses (local context)
    pub hyps: Vec<Hypothesis>,
    /// Goal type to prove
    pub target: String,
}

/// A hypothesis in the local context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub names: Vec<String>,
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Response from `Lean.Widget.getInteractiveGoals`
/// This is a simplified version - the actual response has more fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveGoalsResponse {
    #[serde(default)]
    pub goals: Vec<InteractiveGoal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveGoal {
    #[serde(default)]
    pub hyps: Vec<InteractiveHypothesis>,
    #[serde(rename = "type")]
    pub type_: CodeWithInfos,
    #[serde(default)]
    pub goal_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveHypothesis {
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(rename = "type")]
    pub type_: CodeWithInfos,
    #[serde(default)]
    pub val: Option<CodeWithInfos>,
}

/// Tagged text with semantic info - Lean's CodeWithInfos type
/// Structure: {"tag": [info, content]} or {"text": "..."} or {"append": [...]}
/// See: Lean/Widget/TaggedText.lean
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CodeWithInfos {
    /// Plain text leaf
    Text { text: String },
    /// Tagged span: [info, content] where content is the nested CodeWithInfos
    Tag { tag: (Value, Box<CodeWithInfos>) },
    /// Concatenation of multiple items
    Append { append: Vec<CodeWithInfos> },
}

impl CodeWithInfos {
    /// Extract plain text from the tagged structure
    pub fn to_plain_text(&self) -> String {
        match self {
            CodeWithInfos::Text { text } => text.clone(),
            CodeWithInfos::Tag { tag } => tag.1.to_plain_text(),
            CodeWithInfos::Append { append } => {
                append.iter().map(|item| item.to_plain_text()).collect()
            }
        }
    }
}

impl InteractiveGoal {
    /// Convert to simplified Goal for TUI display
    pub fn to_goal(&self) -> Goal {
        Goal {
            hyps: self
                .hyps
                .iter()
                .map(|h| Hypothesis {
                    names: h.names.clone(),
                    type_: h.type_.to_plain_text(),
                    value: h.val.as_ref().map(|v| v.to_plain_text()),
                })
                .collect(),
            target: self.type_.to_plain_text(),
        }
    }
}

impl InteractiveGoalsResponse {
    /// Convert to simplified goals for TUI
    pub fn to_goals(&self) -> Vec<Goal> {
        self.goals.iter().map(|g| g.to_goal()).collect()
    }
}

/// RPC method names
pub const RPC_CONNECT: &str = "$/lean/rpc/connect";
pub const RPC_KEEP_ALIVE: &str = "$/lean/rpc/keepAlive";
pub const RPC_CALL: &str = "$/lean/rpc/call";
pub const GET_INTERACTIVE_GOALS: &str = "Lean.Widget.getInteractiveGoals";
