//! Component-based UI architecture.

mod definition_header;
mod diff_text;
mod goal_before;
mod goal_section;
mod goal_state;
pub mod goal_tree;
mod header;
mod help_menu;
mod hyp_layer;
mod hyp_section;
mod proof_steps_sidebar;
mod status_bar;
mod tactic_row;
mod tree_builder;
mod tree_colors;
mod tree_hyp_bar;
mod tree_view;

use crossterm::event::{KeyEvent, MouseEvent};
// Re-exports for modes
pub use definition_header::render_definition_header;
pub use diff_text::{diff_style, DiffState, TaggedTextExt};
pub use goal_before::render_goal_before;
pub use goal_section::{GoalSection, GoalSectionInput};
pub use header::Header;
pub use help_menu::HelpMenu;
pub use hyp_section::{HypSection, HypSectionInput};
pub use proof_steps_sidebar::{render_proof_steps_sidebar, ProofStepsSidebarInput};
use ratatui::{layout::Rect, Frame};
pub use status_bar::{StatusBar, StatusBarInput};
pub use tactic_row::render_divider;
pub use tree_view::render_tree_view;

#[derive(Clone)]
pub struct KeyPress(pub KeyEvent);

#[derive(Clone)]
pub enum KeyMouseEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

use crate::lean_rpc::Hypothesis;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectableItem {
    Hypothesis { goal_idx: usize, hyp_idx: usize },
    GoalTarget { goal_idx: usize },
}

#[derive(Debug, Clone)]
pub struct ClickRegion {
    pub area: Rect,
    pub item: SelectableItem,
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct HypothesisFilters {
    pub hide_instances: bool,
    pub hide_inaccessible: bool,
    pub hide_let_values: bool,
    pub reverse_order: bool,
}

/// Filter toggles that modes can support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterToggle {
    Instances,
    Inaccessible,
    LetValues,
    ReverseOrder,
}

impl HypothesisFilters {
    pub fn should_show(self, hyp: &Hypothesis) -> bool {
        if self.hide_instances && hyp.is_instance {
            return false;
        }
        if self.hide_inaccessible && hyp.names.iter().any(|n| n.contains('\u{2020}')) {
            return false;
        }
        true
    }
}

pub fn hypothesis_indices(len: usize, reverse: bool) -> Box<dyn Iterator<Item = usize>> {
    let range = 0..len;
    if reverse {
        Box::new(range.rev())
    } else {
        Box::new(range)
    }
}

pub trait Component {
    type Input;
    /// Use `()` for non-interactive components.
    type Event;

    fn update(&mut self, input: Self::Input);

    fn handle_event(&mut self, _event: Self::Event) -> bool {
        false
    }

    fn render(&mut self, frame: &mut Frame, area: Rect);
}
