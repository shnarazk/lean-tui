//! Paperproof-style proof visualization components.

mod definition_header;
mod goal_before;
mod goal_section;
mod hyp_layer;
mod hyp_section;
mod proof_steps_sidebar;
mod tactic_row;
mod tree_builder;
mod tree_colors;
mod tree_hyp_bar;
mod tree_view;
mod view;

pub use view::{PaperproofView, PaperproofViewInput};
