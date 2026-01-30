//! Component-based UI architecture.

pub mod diff_text;
pub mod goal_section;
pub mod goals_column;
pub mod help_menu;
pub mod hyp_layer;
pub mod hyp_section;
pub mod interactive_widget;
pub mod layout_metrics;
pub mod open_goal_list;
pub mod proof_steps_sidebar;
pub mod render_helpers;
pub mod selection;
pub mod semantic_tableau;
pub mod status_bar;
pub mod tactic_row;
pub mod theme;
pub mod welcome;

pub use crossterm::event::KeyEvent;
use crossterm::event::MouseEvent;
pub use interactive_widget::{InteractiveComponent, InteractiveStatefulWidget};
pub use selection::{ClickRegion, Selection};

#[derive(Clone)]
pub enum KeyMouseEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
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
    /// Toggle a filter setting.
    pub const fn toggle(&mut self, filter: FilterToggle) {
        match filter {
            FilterToggle::Instances => self.hide_instances = !self.hide_instances,
            FilterToggle::Inaccessible => self.hide_inaccessible = !self.hide_inaccessible,
            FilterToggle::LetValues => self.hide_let_values = !self.hide_let_values,
            FilterToggle::ReverseOrder => self.reverse_order = !self.reverse_order,
        }
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
