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
