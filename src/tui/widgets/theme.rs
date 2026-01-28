//! Centralized theme and styling constants.

use ratatui::style::{Color, Modifier, Style};

/// Theme constants for consistent styling across components.
pub struct Theme;

impl Theme {
    // Base text colors
    pub const DIM: Style = Style::new().fg(Color::DarkGray);

    // Special states
    pub const DEPENDENCY: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::DIM);

    // UI chrome colors
    pub const BORDER: Color = Color::DarkGray;
    pub const BORDER_FOCUSED: Color = Color::Cyan;
    pub const TITLE_HYPOTHESIS: Color = Color::Blue;
    pub const TITLE_GOAL: Color = Color::Cyan;

    // Semantic tableau - proof hypothesis colors (green tones)
    pub const PROOF_HYP_BG: Color = Color::Rgb(30, 45, 30);
    pub const PROOF_HYP_FG: Color = Color::Rgb(150, 200, 150);

    // Semantic tableau - data hypothesis colors (yellow/tan tones)
    pub const DATA_HYP_BG: Color = Color::Rgb(50, 45, 25);
    pub const DATA_HYP_FG: Color = Color::Rgb(200, 180, 120);

    // Semantic tableau - node borders
    pub const TACTIC_BORDER: Color = Color::DarkGray;
    pub const CURRENT_NODE_BORDER: Color = Color::Cyan;
    pub const INCOMPLETE_NODE_BORDER: Color = Color::Yellow;
    pub const COMPLETED_NODE_BORDER: Color = Color::Green;

    // Semantic tableau - goals
    pub const GOAL_FG: Color = Color::Rgb(200, 140, 140);
    pub const COMPLETED_GOAL_FG: Color = Color::Green;
}

/// Which pane is currently focused for keyboard navigation.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPane {
    #[default]
    Sidebar,
    Hypotheses,
    Goals,
}

impl FocusedPane {
    /// Cycle to the next pane (Sidebar → Hypotheses → Goals → Sidebar).
    pub const fn next(self) -> Self {
        match self {
            Self::Sidebar => Self::Hypotheses,
            Self::Hypotheses => Self::Goals,
            Self::Goals => Self::Sidebar,
        }
    }

    /// Cycle to the previous pane (reverse of next).
    pub const fn prev(self) -> Self {
        match self {
            Self::Sidebar => Self::Goals,
            Self::Hypotheses => Self::Sidebar,
            Self::Goals => Self::Hypotheses,
        }
    }
}
