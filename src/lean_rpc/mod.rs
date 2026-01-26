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

/// Subexpression semantic information from Lean's `SubexprInfo`.
///
/// This preserves the full structure from Lean, enabling:
/// - Per-subexpression go-to-definition via `info`
/// - Position tracking within expression trees via `subexpr_pos`
/// - Per-subexpression diff highlighting via `diff_status`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubexprInfo {
    /// Reference to `InfoWithCtx` for go-to-definition, hover, etc.
    /// This is an RPC reference that can be passed to `getGoToLocation`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<Value>,

    /// Position within the parent expression tree (`SubExpr.Pos` from Lean).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subexpr_pos: Option<Value>,

    /// Diff status for this subexpression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_status: Option<DiffTag>,
}

impl SubexprInfo {
    /// Parse a `SubexprInfo` from a raw JSON value (as received from Lean).
    fn from_value(value: &Value) -> Self {
        Self {
            info: value.get("info").cloned(),
            subexpr_pos: value.get("subexprPos").cloned(),
            diff_status: value
                .get("diffStatus")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
        }
    }
}

/// Tagged text preserving Lean's `CodeWithInfos` structure with typed info.
///
/// Unlike `CodeWithInfos` which uses opaque `Value` for tag info, this type
/// extracts the semantic information into typed fields, enabling:
/// - Per-subexpression diff highlighting (not just first-found)
/// - Per-subexpression go-to-definition
/// - Future: click-on-subexpression navigation
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

    /// Extract the first `info` field (for go-to-definition).
    pub fn first_info(&self) -> Option<Value> {
        match self {
            Self::Text { .. } => None,
            Self::Tag { info, content } => {
                info.info.clone().or_else(|| content.first_info())
            }
            Self::Append { items } => items.iter().find_map(Self::first_info),
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
    pub session_id: String,
}

/// Simplified goal representation for TUI display.
///
/// Contains structured `TaggedText` for the target, preserving
/// per-subexpression semantic information for diff highlighting
/// and go-to-definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    /// Hypotheses (local context)
    pub hyps: Vec<Hypothesis>,
    /// Goal type to prove (structured with per-subexpression info)
    pub target: TaggedText,
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

/// A hypothesis in the local context.
///
/// Contains structured `TaggedText` for the type (and optional value),
/// preserving per-subexpression semantic information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools, clippy::derive_partial_eq_without_eq)]
pub struct Hypothesis {
    pub names: Vec<String>,
    /// Type of the hypothesis (structured with per-subexpression info)
    pub type_: TaggedText,
    /// Value for let-bindings (e.g., `let x := 5`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub val: Option<TaggedText>,
    /// Is this a typeclass instance hypothesis?
    #[serde(default)]
    pub is_instance: bool,
    /// Is this hypothesis a type?
    #[serde(default)]
    pub is_type: bool,
    /// `FVarIds` for go-to-definition support
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fvar_ids: Option<Vec<String>>,
    /// Hypothesis was inserted (new in current state vs pinned)
    #[serde(default)]
    pub is_inserted: bool,
    /// Hypothesis was removed (gone in current state vs pinned)
    #[serde(default)]
    pub is_removed: bool,
}

impl Hypothesis {
    /// Get the first info from the type (for go-to-definition).
    pub fn first_info(&self) -> Option<Value> {
        self.type_.first_info()
    }
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
    /// Convert to `TaggedText` with typed `SubexprInfo`.
    ///
    /// This preserves the full tree structure while extracting semantic
    /// information into typed fields.
    pub fn to_tagged_text(&self) -> TaggedText {
        match self {
            Self::Text { text } => TaggedText::Text { text: text.clone() },
            Self::Tag { tag } => TaggedText::Tag {
                info: SubexprInfo::from_value(&tag.0),
                content: Box::new(tag.1.to_tagged_text()),
            },
            Self::Append { append } => TaggedText::Append {
                items: append.iter().map(Self::to_tagged_text).collect(),
            },
        }
    }
}

impl InteractiveHypothesis {
    /// Convert to simplified Hypothesis for TUI display.
    ///
    /// Preserves the full `TaggedText` structure for per-subexpression
    /// diff highlighting and go-to-definition.
    pub fn to_hypothesis(&self) -> Hypothesis {
        let val = self.val.as_ref().map(CodeWithInfos::to_tagged_text);
        if let Some(ref v) = val {
            tracing::debug!("Hypothesis {:?} val: {:?}", self.names, v.to_plain_text());
        }
        Hypothesis {
            names: self.names.clone(),
            type_: self.type_.to_tagged_text(),
            val,
            is_instance: self.is_instance.unwrap_or(false),
            is_type: self.is_type.unwrap_or(false),
            fvar_ids: self.fvar_ids.clone(),
            is_inserted: self.is_inserted,
            is_removed: self.is_removed,
        }
    }
}

impl InteractiveGoal {
    /// Convert to simplified Goal for TUI display.
    ///
    /// Preserves the full `TaggedText` structure for per-subexpression
    /// diff highlighting.
    pub fn to_goal(&self) -> Goal {
        Goal {
            hyps: self
                .hyps
                .iter()
                .map(InteractiveHypothesis::to_hypothesis)
                .collect(),
            target: self.type_.to_tagged_text(),
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
            target: self.type_.to_tagged_text(),
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
