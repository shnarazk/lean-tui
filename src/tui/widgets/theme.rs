//! Centralized theme and styling constants.

use ratatui::style::{Color, Modifier, Style};

/// Theme constants for consistent styling across components.
pub struct Theme;

impl Theme {
    // Base text colors
    pub const DIM: Style = Style::new().fg(Color::DarkGray);

    // Special states
    pub const DEPENDENCY: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::DIM);

    // Status indicators
    pub const INSERTED: Style = Style::new().fg(Color::Green);
    pub const REMOVED: Style = Style::new().fg(Color::Red);
    pub const MODIFIED: Style = Style::new().fg(Color::Yellow);

    // UI chrome colors
    pub const BORDER: Color = Color::DarkGray;
    pub const TITLE_HYPOTHESIS: Color = Color::Blue;
    pub const TITLE_GOAL: Color = Color::Cyan;

    // Tree rendering
    pub const TREE_CHARS: Style = Style::new().fg(Color::DarkGray);
    pub const CASE_LABEL: Style = Style::new().fg(Color::Magenta);
    pub const GOAL_NUMBER: Style = Style::new().fg(Color::DarkGray);
}
