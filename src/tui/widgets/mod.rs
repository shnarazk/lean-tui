//! Component-based UI architecture.

pub mod diff_text;
pub mod goal_before;
pub mod goal_box;
pub mod goal_section;
pub mod goal_tree;
pub mod goals_column;
pub mod help_menu;
pub mod hyp_layer;
pub mod hyp_section;
pub mod interactive_widget;
pub mod layout_metrics;
pub mod proof_steps_sidebar;
pub mod render_helpers;
pub mod selection;
pub mod status_bar;
pub mod step_box;
pub mod tactic_row;
pub mod theme;
pub mod tree_builder;
pub mod tree_colors;
pub mod tree_hyp_bar;
pub mod tree_view;

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;

use crate::lean_rpc::Hypothesis;

#[derive(Clone)]
pub enum KeyMouseEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

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
