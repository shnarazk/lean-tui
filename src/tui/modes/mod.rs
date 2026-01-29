//! Display modes for the TUI.

mod before_after;
pub mod deduction_tree;
mod open_goal_list;
mod steps_view;

use std::mem::take;

use before_after::BeforeAfterMode;
pub use before_after::BeforeAfterModeInput;
pub use deduction_tree::DeductionTreeModeInput;
use deduction_tree::SemanticTableau;
use open_goal_list::PlainList;
pub use open_goal_list::PlainListInput;
use ratatui::{layout::Rect, Frame};
pub use steps_view::StepsModeInput;
use steps_view::TacticTree;

use crate::tui::widgets::{
    FilterToggle, HypothesisFilters, InteractiveComponent, KeyMouseEvent, Selection,
};

/// Trait for display modes in the TUI.
pub trait Mode: InteractiveComponent<Input = Self::Model, Event = KeyMouseEvent> {
    type Model;

    const NAME: &'static str;
    const KEYBINDINGS: &'static [(&'static str, &'static str)];
    const SUPPORTED_FILTERS: &'static [FilterToggle];

    fn current_selection(&self) -> Option<Selection>;
}

/// Display mode with embedded state.
#[allow(clippy::large_enum_variant)]
pub enum DisplayMode {
    OpenGoalList(PlainList),
    BeforeAfter(BeforeAfterMode),
    StepsView(TacticTree),
    DeductionTree(SemanticTableau),
}

impl Default for DisplayMode {
    fn default() -> Self {
        Self::DeductionTree(SemanticTableau::default())
    }
}

impl DisplayMode {
    /// Cycle to the next display mode, preserving state.
    pub fn next(&mut self) {
        *self = match take(self) {
            Self::OpenGoalList(_) => Self::BeforeAfter(BeforeAfterMode::default()),
            Self::BeforeAfter(_) => Self::StepsView(TacticTree::default()),
            Self::StepsView(_) => Self::DeductionTree(SemanticTableau::default()),
            Self::DeductionTree(_) => Self::OpenGoalList(PlainList::default()),
        };
    }

    /// Cycle to the previous display mode, preserving state.
    pub fn prev(&mut self) {
        *self = match take(self) {
            Self::OpenGoalList(_) => Self::DeductionTree(SemanticTableau::default()),
            Self::BeforeAfter(_) => Self::OpenGoalList(PlainList::default()),
            Self::StepsView(_) => Self::BeforeAfter(BeforeAfterMode::default()),
            Self::DeductionTree(_) => Self::StepsView(TacticTree::default()),
        };
    }

    /// Get the display name of the current mode.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::OpenGoalList(_) => PlainList::NAME,
            Self::BeforeAfter(_) => BeforeAfterMode::NAME,
            Self::StepsView(_) => TacticTree::NAME,
            Self::DeductionTree(_) => SemanticTableau::NAME,
        }
    }

    /// Get mode-specific keybindings.
    pub const fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::OpenGoalList(_) => PlainList::KEYBINDINGS,
            Self::BeforeAfter(_) => BeforeAfterMode::KEYBINDINGS,
            Self::StepsView(_) => TacticTree::KEYBINDINGS,
            Self::DeductionTree(_) => SemanticTableau::KEYBINDINGS,
        }
    }

    /// Get supported filter toggles.
    pub const fn supported_filters(&self) -> &'static [FilterToggle] {
        match self {
            Self::OpenGoalList(_) => PlainList::SUPPORTED_FILTERS,
            Self::BeforeAfter(_) => BeforeAfterMode::SUPPORTED_FILTERS,
            Self::StepsView(_) => TacticTree::SUPPORTED_FILTERS,
            Self::DeductionTree(_) => SemanticTableau::SUPPORTED_FILTERS,
        }
    }

    /// Get current selection from active mode.
    pub fn current_selection(&self) -> Option<Selection> {
        match self {
            Self::OpenGoalList(m) => m.current_selection(),
            Self::BeforeAfter(m) => m.current_selection(),
            Self::StepsView(m) => m.current_selection(),
            Self::DeductionTree(m) => m.current_selection(),
        }
    }

    /// Get filters from active mode.
    pub const fn filters(&self) -> HypothesisFilters {
        match self {
            Self::OpenGoalList(m) => m.filters(),
            Self::BeforeAfter(m) => m.filters(),
            Self::StepsView(m) => m.filters(),
            Self::DeductionTree(m) => m.filters(),
        }
    }

    /// Whether to show the previous column (`BeforeAfter` mode only).
    pub const fn show_previous(&self) -> bool {
        match self {
            Self::BeforeAfter(m) => m.show_previous(),
            _ => false,
        }
    }

    /// Whether to show the next column (`BeforeAfter` mode only).
    pub const fn show_next(&self) -> bool {
        match self {
            Self::BeforeAfter(m) => m.show_next(),
            _ => false,
        }
    }

    /// Handle event for active mode.
    pub fn handle_event(&mut self, event: KeyMouseEvent) -> bool {
        match self {
            Self::OpenGoalList(m) => m.handle_event(event),
            Self::BeforeAfter(m) => m.handle_event(event),
            Self::StepsView(m) => m.handle_event(event),
            Self::DeductionTree(m) => m.handle_event(event),
        }
    }

    /// Render active mode.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        match self {
            Self::OpenGoalList(m) => m.render(frame, area),
            Self::BeforeAfter(m) => m.render(frame, area),
            Self::StepsView(m) => m.render(frame, area),
            Self::DeductionTree(m) => m.render(frame, area),
        }
    }

    /// Update active mode with appropriate input.
    pub fn update_open_goal_list(&mut self, input: PlainListInput) {
        if let Self::OpenGoalList(m) = self {
            m.update(input);
        }
    }

    pub fn update_before_after(&mut self, input: BeforeAfterModeInput) {
        if let Self::BeforeAfter(m) = self {
            m.update(input);
        }
    }

    pub fn update_steps(&mut self, input: StepsModeInput) {
        if let Self::StepsView(m) = self {
            m.update(input);
        }
    }

    pub fn update_deduction_tree(&mut self, input: DeductionTreeModeInput) {
        if let Self::DeductionTree(m) = self {
            m.update(input);
        }
    }
}
