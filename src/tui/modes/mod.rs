//! Display modes for the TUI.

mod before_after;
mod deduction_tree;
mod goal_tree;
mod steps_view;

pub use before_after::{BeforeAfterMode, BeforeAfterModeInput};
pub use deduction_tree::{DeductionTreeMode, DeductionTreeModeInput};
pub use goal_tree::{GoalTreeMode, GoalTreeModeInput};
pub use steps_view::{StepsMode, StepsModeInput};

use crate::tui::components::{Component, FilterToggle, KeyMouseEvent, SelectableItem};

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
///
/// Each mode has an associated input type (the data model) and provides
/// common functionality like filters, selection, and keybindings.
pub trait Mode: Component<Input = Self::Model, Event = KeyMouseEvent> {
    /// The data model/input type for this mode.
    type Model;

    /// Display name shown in the title bar.
    const NAME: &'static str;

    /// Mode-specific keybindings as (key, description) pairs.
    const KEYBINDINGS: &'static [(&'static str, &'static str)];

    /// Filter toggles supported by this mode.
    const SUPPORTED_FILTERS: &'static [FilterToggle];

    /// Backend data sources used by this mode.
    const BACKENDS: &'static [Backend];

    /// Get currently selected item (if any).
    fn current_selection(&self) -> Option<SelectableItem>;
}

/// Display mode identifier for the TUI.
///
/// Used for cycling between modes and dispatching to the active mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DisplayMode {
    /// Goal tree with hypothesis navigation.
    #[default]
    GoalTree,
    /// Temporal comparison (before/current/after).
    BeforeAfter,
    /// Steps sidebar with hypotheses and goals.
    StepsView,
    /// Paperproof deduction tree visualization.
    DeductionTree,
}

impl DisplayMode {
    /// Cycle to the next display mode.
    pub const fn next(self) -> Self {
        match self {
            Self::GoalTree => Self::BeforeAfter,
            Self::BeforeAfter => Self::StepsView,
            Self::StepsView => Self::DeductionTree,
            Self::DeductionTree => Self::GoalTree,
        }
    }

    /// Cycle to the previous display mode.
    pub const fn prev(self) -> Self {
        match self {
            Self::GoalTree => Self::DeductionTree,
            Self::BeforeAfter => Self::GoalTree,
            Self::StepsView => Self::BeforeAfter,
            Self::DeductionTree => Self::StepsView,
        }
    }

    /// Get the display name of this mode.
    pub const fn name(self) -> &'static str {
        match self {
            Self::GoalTree => GoalTreeMode::NAME,
            Self::BeforeAfter => BeforeAfterMode::NAME,
            Self::StepsView => StepsMode::NAME,
            Self::DeductionTree => DeductionTreeMode::NAME,
        }
    }

    /// Get mode-specific keybindings as (key, description) pairs.
    pub const fn keybindings(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::GoalTree => GoalTreeMode::KEYBINDINGS,
            Self::BeforeAfter => BeforeAfterMode::KEYBINDINGS,
            Self::StepsView => StepsMode::KEYBINDINGS,
            Self::DeductionTree => DeductionTreeMode::KEYBINDINGS,
        }
    }

    /// Get filter toggles supported by this mode.
    pub const fn supported_filters(self) -> &'static [FilterToggle] {
        match self {
            Self::GoalTree => GoalTreeMode::SUPPORTED_FILTERS,
            Self::BeforeAfter => BeforeAfterMode::SUPPORTED_FILTERS,
            Self::StepsView => StepsMode::SUPPORTED_FILTERS,
            Self::DeductionTree => DeductionTreeMode::SUPPORTED_FILTERS,
        }
    }

    /// Get backend data sources used by this mode.
    pub const fn backends(self) -> &'static [Backend] {
        match self {
            Self::GoalTree => GoalTreeMode::BACKENDS,
            Self::BeforeAfter => BeforeAfterMode::BACKENDS,
            Self::StepsView => StepsMode::BACKENDS,
            Self::DeductionTree => DeductionTreeMode::BACKENDS,
        }
    }

    /// Format backends as a display string (e.g., "Paperproof | tree-sitter").
    pub fn backends_display(self) -> String {
        self.backends()
            .iter()
            .map(|b| b.name())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}
