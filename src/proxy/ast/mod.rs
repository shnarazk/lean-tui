//! AST analysis for Lean proofs using tree-sitter.
//!
//! Provides utilities for analyzing proof structure:
//! - Definition discovery (theorem, lemma, def, example)
//! - Case-split detection (`by_cases`, `cases`, `induction`, etc.)
//! - Tactic enumeration
//! - Proof navigation (previous/next tactic)

mod case_splits;
mod definitions;
mod navigation;
mod tactics;
mod util;

pub use case_splits::{find_case_splits, CaseSplitInfo};
pub use definitions::{find_enclosing_definition, DefinitionInfo};
pub use navigation::{find_next_tactic, find_previous_tactic};
pub use tactics::{find_all_tactics_in_proof, TacticInfo};

#[cfg(test)]
mod tests;
