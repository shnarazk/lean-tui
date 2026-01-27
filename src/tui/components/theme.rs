//! Centralized theme and styling constants.

use ratatui::style::{Color, Modifier, Style};

/// Theme constants for consistent styling across components.
pub struct Theme;

impl Theme {
    // Base text colors
    pub const DIM: Style = Style::new().fg(Color::DarkGray);
    pub const NORMAL: Style = Style::new().fg(Color::White);

    // Selection highlighting
    pub const SELECTION: Style = Style::new().bg(Color::Rgb(40, 40, 60)).fg(Color::White);

    // Special states
    pub const DEPENDENCY: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::DIM);

    pub const SPAWNED_GOAL: Style = Style::new().bg(Color::Rgb(20, 40, 20));

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

    /// Get alternating row background color.
    #[must_use]
    pub const fn row_bg(index: usize) -> Color {
        match index % 2 {
            0 => Color::Rgb(20, 20, 20),
            _ => Color::Rgb(25, 25, 25),
        }
    }
}
