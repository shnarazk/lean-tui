//! Component-based UI architecture.

mod diff_text;
mod goal_state;
mod goal_tree;
mod goal_view;
mod header;
mod help_menu;
mod paperproof;
mod status_bar;

use crossterm::event::{KeyEvent, MouseEvent};
pub use goal_view::{GoalView, GoalViewInput};
pub use header::Header;
pub use help_menu::HelpMenu;
pub use paperproof::{PaperproofView, PaperproofViewInput};
use ratatui::{layout::Rect, Frame};
pub use status_bar::StatusBar;

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

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct HypothesisFilters {
    pub hide_instances: bool,
    pub hide_types: bool,
    pub hide_inaccessible: bool,
    pub hide_let_values: bool,
    pub reverse_order: bool,
    pub hide_definition: bool,
}

impl Default for HypothesisFilters {
    fn default() -> Self {
        Self {
            hide_instances: false,
            hide_types: false,
            hide_inaccessible: false,
            hide_let_values: false,
            reverse_order: false,
            hide_definition: true,
        }
    }
}

impl HypothesisFilters {
    pub fn should_show(self, hyp: &Hypothesis) -> bool {
        if self.hide_instances && hyp.is_instance {
            return false;
        }
        if self.hide_types && hyp.is_type {
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
