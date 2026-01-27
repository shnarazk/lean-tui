//! AST analysis for Lean proofs using tree-sitter.
//!
//! Provides utilities for analyzing proof structure:
//! - Definition discovery (theorem, lemma, def, example)
//! - Tactic enumeration
//! - Proof navigation (previous/next tactic)

mod definitions;
mod navigation;
mod tactics;

pub use definitions::{find_enclosing_definition, DefinitionInfo};
pub use navigation::{find_next_tactic, find_previous_tactic};
pub use tactics::{find_all_tactics_in_proof, TacticInfo};

#[cfg(test)]
mod tests;
