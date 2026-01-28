//! Unified proof DAG - single source of truth for all display modes.
//!
//! This module defines the `ProofDag` data structure that contains all semantic
//! information about a proof, pre-computed by the proxy. Each display mode uses
//! the subset of data it needs, and the TUI only handles layout and rendering.

mod from_local;
mod from_paperproof;
mod goto_resolution;
pub mod node;
pub mod state;
mod structure;

use async_lsp::lsp_types::Position;
pub use node::ProofDagNode;
use serde::{Deserialize, Serialize};
pub use state::{HypothesisInfo, ProofState};

/// Unique identifier for a node in the proof DAG.
pub type NodeId = u32;

/// The complete proof DAG - single source of truth for all display modes.
/// Contains all semantic information pre-computed by the proxy.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    /// Source of the DAG data.
    pub source: ProofDagSource,
}

/// Source of proof DAG data.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProofDagSource {
    /// From Paperproof Lean library RPC.
    Paperproof,
    /// From local tree-sitter analysis.
    #[default]
    Local,
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

    /// Find and set the current node (closest to cursor).
    pub(crate) fn set_current_node(&mut self, cursor: Position) {
        self.current_node = self
            .nodes
            .iter()
            .map(|node| {
                let line_diff = (i64::from(node.position.line) - i64::from(cursor.line)).abs();
                let char_diff =
                    (i64::from(node.position.character) - i64::from(cursor.character)).abs();
                let penalty = if node.position.line > cursor.line {
                    10000
                } else {
                    0
                };
                (node.id, line_diff * 1000 + char_diff + penalty)
            })
            .min_by_key(|(_, dist)| *dist)
            .map(|(id, _)| id);
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
