//! Lean RPC protocol types and client for communication with lake serve.
//!
//! Lean uses custom RPC methods tunneled through LSP:
//! - `$/lean/rpc/connect` - establish session
//! - `$/lean/rpc/keepAlive` - maintain session
//! - `$/lean/rpc/call` - invoke Lean methods like `getInteractiveGoals`

mod client;

use async_lsp::lsp_types;
pub use client::{GoToKind, RpcClient};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// Response from `$/lean/rpc/connect`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcConnectResponse {
    pub session_id: String,
}

/// Simplified goal representation for TUI display
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    /// Hypotheses (local context)
    pub hyps: Vec<Hypothesis>,
    /// Goal type to prove
    pub target: String,
    /// Prefix before the target (usually `⊢ `, but `∣ ` for conv goals)
    #[serde(default)]
    pub prefix: String,
    /// Case label (e.g., `case foo` for pattern matching)
    #[serde(default)]
    pub user_name: Option<String>,
    /// Goal was inserted (new in current state vs pinned)
    #[serde(default)]
    pub is_inserted: bool,
    /// Goal was removed (gone in current state vs pinned)
    #[serde(default)]
    pub is_removed: bool,
}

/// A hypothesis in the local context
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools, clippy::derive_partial_eq_without_eq)]
pub struct Hypothesis {
    pub names: Vec<String>,
    pub type_: String,
    /// Value for let-bindings (e.g., `let x := 5`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub val: Option<String>,
    /// Is this a typeclass instance hypothesis?
    #[serde(default)]
    pub is_instance: bool,
    /// Is this hypothesis a type?
    #[serde(default)]
    pub is_type: bool,
    /// `FVarIds` for go-to-definition support
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fvar_ids: Option<Vec<String>>,
    /// First `SubexprInfo` from type (`InfoWithCtx` reference for
    /// `getGoToLocation`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<Value>,
    /// Hypothesis was inserted (new in current state vs pinned)
    #[serde(default)]
    pub is_inserted: bool,
    /// Hypothesis was removed (gone in current state vs pinned)
    #[serde(default)]
    pub is_removed: bool,
    /// Diff status from the type's first subexpression
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_status: Option<DiffTag>,
}

/// Response from `Lean.Widget.getInteractiveGoals`
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
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub is_inserted: bool,
    #[serde(default)]
    pub is_removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveHypothesis {
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub fvar_ids: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub type_: CodeWithInfos,
    #[serde(default)]
    pub val: Option<CodeWithInfos>,
    #[serde(default)]
    pub is_instance: Option<bool>,
    #[serde(default)]
    pub is_type: Option<bool>,
    #[serde(default)]
    pub is_inserted: bool,
    #[serde(default)]
    pub is_removed: bool,
}

/// Tagged text with semantic info - Lean's `CodeWithInfos` type.
/// Structure: {"tag": [info, content]} or {"text": "..."} or {"append": [...]}
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

    /// Extract the first `diffStatus` from the tagged text.
    /// Returns the diff tag if any subexpression has been marked as changed.
    pub fn first_diff_status(&self) -> Option<DiffTag> {
        match self {
            Self::Text { .. } => None,
            Self::Tag { tag } => {
                // tag.0 is the SubexprInfo which may contain {"diffStatus": "wasChanged", ...}
                tag.0.get("diffStatus").map_or_else(
                    || tag.1.first_diff_status(),
                    |status| serde_json::from_value(status.clone()).ok(),
                )
            }
            Self::Append { append } => append.iter().find_map(Self::first_diff_status),
        }
    }
}

impl InteractiveHypothesis {
    /// Convert to simplified Hypothesis for TUI display
    pub fn to_hypothesis(&self) -> Hypothesis {
        let val = self.val.as_ref().map(CodeWithInfos::to_plain_text);
        if let Some(ref v) = val {
            tracing::debug!("Hypothesis {:?} val: {:?}", self.names, v);
        }
        Hypothesis {
            names: self.names.clone(),
            type_: self.type_.to_plain_text(),
            val,
            is_instance: self.is_instance.unwrap_or(false),
            is_type: self.is_type.unwrap_or(false),
            fvar_ids: self.fvar_ids.clone(),
            info: self.type_.first_subexpr_info(),
            is_inserted: self.is_inserted,
            is_removed: self.is_removed,
            diff_status: self.type_.first_diff_status(),
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
                .map(InteractiveHypothesis::to_hypothesis)
                .collect(),
            target: self.type_.to_plain_text(),
            prefix: if self.goal_prefix.is_empty() {
                "⊢ ".to_string()
            } else {
                self.goal_prefix.clone()
            },
            user_name: self.user_name.clone(),
            is_inserted: self.is_inserted,
            is_removed: self.is_removed,
        }
    }
}

impl InteractiveGoalsResponse {
    /// Convert to simplified goals for TUI
    pub fn to_goals(&self) -> Vec<Goal> {
        self.goals.iter().map(InteractiveGoal::to_goal).collect()
    }
}

/// Response from `Lean.Widget.getInteractiveTermGoal`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveTermGoalResponse {
    #[serde(default)]
    pub hyps: Vec<InteractiveHypothesis>,
    #[serde(rename = "type")]
    pub type_: CodeWithInfos,
    /// Syntactic range of the term
    #[serde(default)]
    pub range: Option<lsp_types::Range>,
}

impl InteractiveTermGoalResponse {
    /// Convert to Goal with "Expected" marker for unified display.
    pub fn to_goal(&self) -> Goal {
        Goal {
            hyps: self
                .hyps
                .iter()
                .map(InteractiveHypothesis::to_hypothesis)
                .collect(),
            target: self.type_.to_plain_text(),
            prefix: "⊢ ".to_string(),
            user_name: Some("Expected".to_string()), // Marker for term goal
            is_inserted: false,
            is_removed: false,
        }
    }
}

pub const RPC_CONNECT: &str = "$/lean/rpc/connect";
pub const RPC_CALL: &str = "$/lean/rpc/call";
pub const GET_INTERACTIVE_GOALS: &str = "Lean.Widget.getInteractiveGoals";
pub const GET_INTERACTIVE_TERM_GOAL: &str = "Lean.Widget.getInteractiveTermGoal";
pub const GET_GOTO_LOCATION: &str = "Lean.Widget.getGoToLocation";
