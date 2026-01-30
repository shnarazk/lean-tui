//! AST analysis for Lean proofs using tree-sitter.
//!
//! Provides utilities for analyzing proof structure:
//! - Definition discovery (theorem, lemma, def, example)
//! - Tactic enumeration
//! - Proof navigation (previous/next tactic)

mod definitions;
mod navigation;

pub use definitions::DefinitionInfo;
pub use navigation::{find_next_tactic, find_previous_tactic};

#[cfg(test)]
mod tests;
