//! Proof state types - goals and hypotheses at a point in the proof.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::super::{Goal, GotoLocations, TaggedText};

/// Check if a name is a Lean 4 hygienic macro identifier.
/// These contain `._hyg.` or `._@.` patterns and are internal implementation
/// details.
fn is_hygienic_name(name: &str) -> bool {
    name.contains("._hyg.") || name.contains("._@.")
}

/// User-visible name for a goal.
///
/// Serializes to/from `Option<String>` (null or a string) for compatibility
/// with the Lean server which sends `username: null` or `username: "name"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum UserName {
    /// No user-visible name (anonymous goal).
    #[default]
    Anonymous,
    /// A named goal (e.g., "case inl", "h").
    Named(String),
}

impl Serialize for UserName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Anonymous => serializer.serialize_none(),
            Self::Named(name) => serializer.serialize_some(name),
        }
    }
}

impl<'de> Deserialize<'de> for UserName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        Ok(opt.map_or(Self::Anonymous, |name| Self::from_raw(&name)))
    }
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
#[serde(rename_all = "camelCase")]
pub struct ProofState {
    pub goals: Vec<GoalInfo>,
    pub hypotheses: Vec<HypothesisInfo>,
}

/// A goal to prove.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GoalInfo {
    /// Goal type expression (with diff highlighting).
    #[serde(rename = "type")]
    pub type_: TaggedText,
    /// User-visible name (e.g., "case inl").
    pub username: UserName,
    /// Internal goal ID (for tracking across steps).
    pub id: String,
    /// Whether this goal was removed (for diff display in "before" view).
    #[serde(default)]
    pub is_removed: bool,
    /// Pre-resolved goto locations for navigation.
    #[serde(default)]
    pub goto_locations: GotoLocations,
}

/// A hypothesis in scope.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HypothesisInfo {
    /// User-visible name.
    pub name: String,
    /// Type expression (with diff highlighting).
    #[serde(rename = "type")]
    pub type_: TaggedText,
    /// Value for let-bindings (with diff highlighting).
    pub value: Option<TaggedText>,
    /// Internal ID for tracking.
    pub id: String,
    /// Whether this hypothesis is a proof term.
    pub is_proof: bool,
    /// Whether this is a type class instance.
    pub is_instance: bool,
    /// Whether this hypothesis was removed (for diff display in "before" view).
    #[serde(default)]
    pub is_removed: bool,
    /// Pre-resolved goto locations for navigation.
    #[serde(default)]
    pub goto_locations: GotoLocations,
}

// ============================================================================
// Conversions (for local fallback)
// ============================================================================

impl ProofState {
    pub fn from_goals(goals: &[Goal]) -> Self {
        Self {
            goals: goals
                .iter()
                .map(|g| GoalInfo {
                    type_: g.target.clone(),
                    username: UserName::from_optional(g.user_name.as_deref()),
                    id: String::new(),
                    is_removed: false,
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
                            type_: h.type_.clone(),
                            value: h.val.clone(),
                            id: String::new(),
                            is_proof: false,
                            is_instance: h.is_instance,
                            is_removed: false,
                            goto_locations: h.goto_locations.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}
