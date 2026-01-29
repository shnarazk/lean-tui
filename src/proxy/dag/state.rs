//! Proof state types - goals and hypotheses at a point in the proof.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::lean_rpc::{Goal, GotoLocations, PaperproofGoalInfo, PaperproofHypothesis, TaggedText};

/// Check if a name is a Lean 4 hygienic macro identifier.
/// These contain `._hyg.` or `._@.` patterns and are internal implementation
/// details.
fn is_hygienic_name(name: &str) -> bool {
    name.contains("._hyg.") || name.contains("._@.")
}

/// User-visible name for a goal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum UserName {
    /// No user-visible name (anonymous goal).
    #[default]
    Anonymous,
    /// A named goal (e.g., "case inl", "h").
    Named(String),
}

impl fmt::Display for UserName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anonymous => write!(f, ""),
            Self::Named(n) => write!(f, "{n}"),
        }
    }
}

impl UserName {
    /// Create from a raw string, filtering hygienic names and "[anonymous]".
    pub fn from_raw(name: &str) -> Self {
        if name.is_empty() || name == "[anonymous]" || is_hygienic_name(name) {
            Self::Anonymous
        } else {
            Self::Named(name.to_string())
        }
    }

    /// Create from an optional string.
    pub fn from_optional(name: Option<&str>) -> Self {
        name.map_or(Self::Anonymous, Self::from_raw)
    }

    /// Get the name if present.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Anonymous => None,
            Self::Named(n) => Some(n),
        }
    }
}

/// A proof state (goals and hypotheses at a point in the proof).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProofState {
    pub goals: Vec<GoalInfo>,
    pub hypotheses: Vec<HypothesisInfo>,
}

/// A goal to prove.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoalInfo {
    /// Goal type expression.
    pub type_: String,
    /// User-visible name (e.g., "case inl").
    pub username: UserName,
    /// Internal goal ID (for tracking across steps).
    pub id: String,
    /// Pre-resolved goto locations for navigation.
    #[serde(default)]
    pub goto_locations: GotoLocations,
}

/// A hypothesis in scope.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HypothesisInfo {
    /// User-visible name.
    pub name: String,
    /// Type expression.
    pub type_: String,
    /// Value for let-bindings.
    pub value: Option<String>,
    /// Internal ID for tracking.
    pub id: String,
    /// Whether this hypothesis is a proof term.
    pub is_proof: bool,
    /// Whether this is a type class instance.
    pub is_instance: bool,
    /// Pre-resolved goto locations for navigation.
    #[serde(default)]
    pub goto_locations: GotoLocations,
}

// ============================================================================
// Conversions
// ============================================================================

impl From<&PaperproofGoalInfo> for GoalInfo {
    fn from(goal: &PaperproofGoalInfo) -> Self {
        Self {
            type_: goal.type_.clone(),
            username: UserName::from_raw(&goal.username),
            id: goal.id.clone(),
            goto_locations: GotoLocations::default(),
        }
    }
}

impl From<&PaperproofHypothesis> for HypothesisInfo {
    fn from(h: &PaperproofHypothesis) -> Self {
        let name = if is_hygienic_name(&h.username) {
            String::new()
        } else {
            h.username.clone()
        };
        Self {
            name,
            type_: h.type_.clone(),
            value: h.value.clone(),
            id: h.id.clone(),
            is_proof: h.is_proof == "proof",
            is_instance: false,
            goto_locations: GotoLocations::default(),
        }
    }
}

impl From<&PaperproofGoalInfo> for ProofState {
    fn from(goal: &PaperproofGoalInfo) -> Self {
        Self {
            goals: vec![goal.into()],
            hypotheses: goal.hyps.iter().map(HypothesisInfo::from).collect(),
        }
    }
}

impl ProofState {
    pub(super) fn from_goals_after(goals_after: &[PaperproofGoalInfo]) -> Self {
        goals_after.first().map_or_else(Self::default, |goal| Self {
            goals: goals_after.iter().map(GoalInfo::from).collect(),
            hypotheses: goal.hyps.iter().map(HypothesisInfo::from).collect(),
        })
    }

    pub fn from_goals(goals: &[Goal]) -> Self {
        Self {
            goals: goals
                .iter()
                .map(|g| GoalInfo {
                    type_: g.target.to_plain_text(),
                    username: UserName::from_optional(g.user_name.as_deref()),
                    id: String::new(),
                    goto_locations: g.goto_locations.clone(),
                })
                .collect(),
            hypotheses: goals
                .first()
                .map(|g| {
                    g.hyps
                        .iter()
                        .map(|h| HypothesisInfo {
                            name: h.names.join(", "),
                            type_: h.type_.to_plain_text(),
                            value: h.val.as_ref().map(TaggedText::to_plain_text),
                            id: String::new(),
                            is_proof: false,
                            is_instance: h.is_instance,
                            goto_locations: h.goto_locations.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}
