//! Unified proof DAG - single source of truth for all display modes.
//!
//! This module defines the `ProofDag` data structure that contains all semantic
//! information about a proof, pre-computed by the server. Each display mode
//! uses the subset of data it needs, and the TUI only handles layout and
//! rendering.

pub mod node;
pub mod state;

pub use node::ProofDagNode;
use serde::{Deserialize, Serialize};
pub use state::{GoalInfo, HypothesisInfo, ProofState};

/// Unique identifier for a node in the proof DAG.
pub type NodeId = u32;

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

// ============================================================================
// Navigation methods
// ============================================================================

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
