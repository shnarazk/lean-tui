//! Display modes for the TUI.

mod before_after;
mod deduction_tree;
mod goal_tree;
mod steps_view;

use std::mem::take;

use before_after::BeforeAfterMode;
pub use before_after::BeforeAfterModeInput;
use deduction_tree::DeductionTreeMode;
pub use deduction_tree::DeductionTreeModeInput;
use goal_tree::GoalTreeMode;
pub use goal_tree::GoalTreeModeInput;
use ratatui::{layout::Rect, Frame};
use steps_view::StepsMode;
pub use steps_view::StepsModeInput;

use crate::tui::widgets::{
    interactive_widget::InteractiveWidget, FilterToggle, HypothesisFilters, KeyMouseEvent,
    SelectableItem,
};

/// Backend data source for a display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Goals from Lean RPC (getInteractiveGoals).
    LeanRpc,
    /// Proof steps from Paperproof library.
    Paperproof,
    /// Proof steps from local tree-sitter analysis.
    TreeSitter,
}

impl Backend {
    /// Get the display name of this backend.
    pub const fn name(self) -> &'static str {
        match self {
            Self::LeanRpc => "Lean RPC",
            Self::Paperproof => "Paperproof",
            Self::TreeSitter => "tree-sitter",
        }
    }
}

/// Trait for display modes in the TUI.
pub trait Mode: InteractiveWidget<Input = Self::Model, Event = KeyMouseEvent> {
    type Model;

    const NAME: &'static str;
    const KEYBINDINGS: &'static [(&'static str, &'static str)];
    const SUPPORTED_FILTERS: &'static [FilterToggle];
    const BACKENDS: &'static [Backend];

    fn current_selection(&self) -> Option<SelectableItem>;
}

/// Display mode with embedded state.
#[allow(clippy::large_enum_variant)]
pub enum DisplayMode {
    GoalTree(GoalTreeMode),
    BeforeAfter(BeforeAfterMode),
    StepsView(StepsMode),
    DeductionTree(DeductionTreeMode),
}

impl Default for DisplayMode {
    fn default() -> Self {
        Self::GoalTree(GoalTreeMode::default())
    }
}

impl DisplayMode {
    /// Cycle to the next display mode, preserving state.
    pub fn next(&mut self) {
        *self = match take(self) {
            Self::GoalTree(_) => Self::BeforeAfter(BeforeAfterMode::default()),
            Self::BeforeAfter(_) => Self::StepsView(StepsMode::default()),
            Self::StepsView(_) => Self::DeductionTree(DeductionTreeMode::default()),
            Self::DeductionTree(_) => Self::GoalTree(GoalTreeMode::default()),
        };
    }

    /// Cycle to the previous display mode, preserving state.
    pub fn prev(&mut self) {
        *self = match take(self) {
            Self::GoalTree(_) => Self::DeductionTree(DeductionTreeMode::default()),
            Self::BeforeAfter(_) => Self::GoalTree(GoalTreeMode::default()),
            Self::StepsView(_) => Self::BeforeAfter(BeforeAfterMode::default()),
            Self::DeductionTree(_) => Self::StepsView(StepsMode::default()),
        };
    }

    /// Get the display name of the current mode.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::GoalTree(_) => GoalTreeMode::NAME,
            Self::BeforeAfter(_) => BeforeAfterMode::NAME,
            Self::StepsView(_) => StepsMode::NAME,
            Self::DeductionTree(_) => DeductionTreeMode::NAME,
        }
    }

    /// Get mode-specific keybindings.
    pub const fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::GoalTree(_) => GoalTreeMode::KEYBINDINGS,
            Self::BeforeAfter(_) => BeforeAfterMode::KEYBINDINGS,
            Self::StepsView(_) => StepsMode::KEYBINDINGS,
            Self::DeductionTree(_) => DeductionTreeMode::KEYBINDINGS,
        }
    }

    /// Get supported filter toggles.
    pub const fn supported_filters(&self) -> &'static [FilterToggle] {
        match self {
            Self::GoalTree(_) => GoalTreeMode::SUPPORTED_FILTERS,
            Self::BeforeAfter(_) => BeforeAfterMode::SUPPORTED_FILTERS,
            Self::StepsView(_) => StepsMode::SUPPORTED_FILTERS,
            Self::DeductionTree(_) => DeductionTreeMode::SUPPORTED_FILTERS,
        }
    }

    /// Get backend data sources.
    pub const fn backends(&self) -> &'static [Backend] {
        match self {
            Self::GoalTree(_) => GoalTreeMode::BACKENDS,
            Self::BeforeAfter(_) => BeforeAfterMode::BACKENDS,
            Self::StepsView(_) => StepsMode::BACKENDS,
            Self::DeductionTree(_) => DeductionTreeMode::BACKENDS,
        }
    }

    /// Format backends as display string.
    pub fn backends_display(&self) -> String {
        self.backends()
            .iter()
            .map(|b| b.name())
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// Get current selection from active mode.
    pub fn current_selection(&self) -> Option<SelectableItem> {
        match self {
            Self::GoalTree(m) => m.current_selection(),
            Self::BeforeAfter(m) => m.current_selection(),
            Self::StepsView(m) => m.current_selection(),
            Self::DeductionTree(m) => m.current_selection(),
        }
    }

    /// Get filters from active mode.
    pub const fn filters(&self) -> HypothesisFilters {
        match self {
            Self::GoalTree(m) => m.filters(),
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
            Self::GoalTree(m) => m.handle_event(event),
            Self::BeforeAfter(m) => m.handle_event(event),
            Self::StepsView(m) => m.handle_event(event),
            Self::DeductionTree(m) => m.handle_event(event),
        }
    }

    /// Render active mode.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        match self {
            Self::GoalTree(m) => m.render(frame, area),
            Self::BeforeAfter(m) => m.render(frame, area),
            Self::StepsView(m) => m.render(frame, area),
            Self::DeductionTree(m) => m.render(frame, area),
        }
    }

    /// Update active mode with appropriate input.
    pub fn update_goal_tree(&mut self, input: GoalTreeModeInput) {
        if let Self::GoalTree(m) = self {
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
