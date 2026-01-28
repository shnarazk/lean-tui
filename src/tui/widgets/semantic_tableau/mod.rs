//! Semantic tableau widget - Paperproof-style proof tree visualization.

mod canvas;
pub mod given_pane;
pub mod layout;
pub mod navigation;
pub mod proof_pane;
pub mod theorem_pane;
pub mod tree_layout;

pub use layout::{SemanticTableauLayout, SemanticTableauState};
pub use super::{ClickRegion, Selection};
