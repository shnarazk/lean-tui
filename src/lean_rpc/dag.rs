//! Unified proof DAG - single source of truth for all display modes.
//!
//! This module defines the `ProofDag` data structure that contains all semantic
//! information about a proof, pre-computed by the server. Each display mode
//! uses the subset of data it needs, and the TUI only handles layout and
//! rendering.

use std::fmt;

use async_lsp::lsp_types::Position;
use serde::{Deserialize, Serialize};

use super::{GotoLocations, TaggedText};

// ============================================================================
// Node ID
// ============================================================================

/// Unique identifier for a node in the proof DAG.
pub type NodeId = u32;

// ============================================================================
// Proof State Types
// ============================================================================

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
// Proof DAG Node
// ============================================================================

/// A node representing a proof state after applying a tactic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofDagNode {
    pub id: NodeId,

    // === Tactic information ===
    pub tactic: DagTacticInfo,
    pub position: Position,

    // === State before/after (for BeforeAfter mode) ===
    pub state_before: ProofState,
    pub state_after: ProofState,

    // === Diff information (pre-computed) ===
    /// Indices into `state_after.hypotheses` for new hypotheses.
    pub new_hypotheses: Vec<usize>,

    // === Tree structure (for DeductionTree mode) ===
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub depth: usize,

    /// True if this node has spawned goals (e.g., inline `by` blocks) that are
    /// not solved.
    #[serde(default)]
    pub has_unsolved_spawned_goals: bool,
}

impl ProofDagNode {
    /// All subgoals solved (`goals_after` empty).
    pub const fn is_complete(&self) -> bool {
        self.state_after.goals.is_empty()
    }

    /// No children in tree.
    pub const fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

/// Information about a tactic application.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DagTacticInfo {
    /// The tactic text (e.g., "intro n").
    pub text: String,
    /// Hypothesis names used by this tactic.
    pub depends_on: Vec<String>,
    /// Theorem names referenced by this tactic.
    pub theorems_used: Vec<String>,
}

// ============================================================================
// Proof DAG
// ============================================================================

/// The complete proof DAG - single source of truth for all display modes.
/// Contains all semantic information pre-computed by the server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProofDag {
    /// All nodes indexed by `NodeId`.
    pub nodes: Vec<ProofDagNode>,

    /// Root node ID (first tactic).
    pub root: Option<NodeId>,

    /// Node closest to cursor position.
    pub current_node: Option<NodeId>,

    /// Initial proof state (theorem hypotheses and goal).
    pub initial_state: ProofState,

    /// Metadata about the proof.
    pub definition_name: Option<String>,

    /// Orphan nodes not connected to main tree (e.g., inline `by` blocks).
    /// These should be rendered separately.
    #[serde(default)]
    pub orphans: Vec<NodeId>,
}

impl ProofDag {
    /// Get a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&ProofDagNode> {
        self.nodes.get(id as usize)
    }

    /// Iterate nodes in depth-first order (for `StepsView`).
    pub fn dfs_iter(&self) -> impl Iterator<Item = &ProofDagNode> {
        DfsIterator {
            dag: self,
            stack: self.root.into_iter().collect(),
        }
    }

    /// Check if the DAG is empty.
    pub const fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get number of nodes.
    pub const fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if a node is the current node (closest to cursor).
    pub fn is_current(&self, node_id: NodeId) -> bool {
        self.current_node == Some(node_id)
    }
}

/// Depth-first iterator over proof DAG nodes.
struct DfsIterator<'a> {
    dag: &'a ProofDag,
    stack: Vec<NodeId>,
}

impl<'a> Iterator for DfsIterator<'a> {
    type Item = &'a ProofDagNode;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let node = self.dag.get(id)?;
        // Push children in reverse order so first child is processed first
        for &child_id in node.children.iter().rev() {
            self.stack.push(child_id);
        }
        Some(node)
    }
}
