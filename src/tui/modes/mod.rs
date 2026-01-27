//! Display modes for the TUI.

mod before_after;
mod deduction_tree;
mod goal_tree;
mod steps_view;

pub use before_after::{BeforeAfterMode, BeforeAfterModeInput};
pub use deduction_tree::{DeductionTreeMode, DeductionTreeModeInput};
pub use goal_tree::{GoalTreeMode, GoalTreeModeInput};
pub use steps_view::{StepsMode, StepsModeInput};

/// Display mode for the TUI.
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
            Self::GoalTree => "Goal Tree",
            Self::BeforeAfter => "Before/After",
            Self::StepsView => "Steps",
            Self::DeductionTree => "Deduction Tree",
        }
    }
}
