//! Lean RPC protocol types and client for communication with lake serve.
//!
//! Lean uses custom RPC methods tunneled through LSP:
//! - `$/lean/rpc/connect` - establish session
//! - `$/lean/rpc/keepAlive` - maintain session
//! - `$/lean/rpc/call` - invoke Lean methods like `getInteractiveGoals`

mod goal_fetcher;
mod rpc_client;

pub use goal_fetcher::spawn_goal_fetch;
pub use rpc_client::RpcClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Response from `$/lean/rpc/connect`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcConnectResponse {
    pub session_id: String,
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
    /// `FVarIds` for go-to-definition support
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fvar_ids: Option<Vec<String>>,
    /// First `SubexprInfo` from type (`InfoWithCtx` reference for
    /// `getGoToLocation`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<Value>,
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
#[serde(rename_all = "camelCase")]
pub struct InteractiveHypothesis {
    #[serde(default)]
    pub names: Vec<String>,
    /// `FVarIds` for each hypothesis in the bundle (same length as names)
    #[serde(default)]
    pub fvar_ids: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub type_: CodeWithInfos,
    #[serde(default)]
    pub val: Option<CodeWithInfos>,
}

/// Tagged text with semantic info - Lean's `CodeWithInfos` type
/// Structure: {"tag": [info, content]} or {"text": "..."} or {"append": [...]}
/// See: Lean/Widget/TaggedText.lean
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CodeWithInfos {
    /// Plain text leaf
    Text { text: String },
    /// Tagged span: [info, content] where content is the nested `CodeWithInfos`
    Tag { tag: (Value, Box<Self>) },
    /// Concatenation of multiple items
    Append { append: Vec<Self> },
}

impl CodeWithInfos {
    /// Extract plain text from the tagged structure
    pub fn to_plain_text(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::Tag { tag } => tag.1.to_plain_text(),
            Self::Append { append } => append.iter().map(Self::to_plain_text).collect(),
        }
    }

    /// Extract the first `SubexprInfo` (`InfoWithCtx` reference) from the
    /// tagged text. This can be used with `getGoToLocation` to jump to the
    /// definition.
    pub fn first_subexpr_info(&self) -> Option<Value> {
        match self {
            Self::Text { .. } => None,
            Self::Tag { tag } => {
                // tag.0 is the SubexprInfo which contains {"info": InfoWithCtx, "subexprPos":
                // ...} We need the "info" field for getGoToLocation
                tag.0
                    .get("info")
                    .cloned()
                    .or_else(|| tag.1.first_subexpr_info())
            }
            Self::Append { append } => append.iter().find_map(Self::first_subexpr_info),
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
                    fvar_ids: h.fvar_ids.clone(),
                    info: h.type_.first_subexpr_info(),
                })
                .collect(),
            target: self.type_.to_plain_text(),
        }
    }
}

impl InteractiveGoalsResponse {
    /// Convert to simplified goals for TUI
    pub fn to_goals(&self) -> Vec<Goal> {
        self.goals.iter().map(InteractiveGoal::to_goal).collect()
    }
}

/// RPC method names
pub const RPC_CONNECT: &str = "$/lean/rpc/connect";
pub const RPC_CALL: &str = "$/lean/rpc/call";
pub const GET_INTERACTIVE_GOALS: &str = "Lean.Widget.getInteractiveGoals";
pub const GET_GO_TO_LOCATION: &str = "Lean.Widget.getGoToLocation";
