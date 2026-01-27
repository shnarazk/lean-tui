//! Color palette for the proof tree view, matching Paperproof's visual style.

use ratatui::style::Color;

pub const HYPOTHESIS_BG: Color = Color::Rgb(60, 80, 60);
pub const HYPOTHESIS_FG: Color = Color::Rgb(200, 230, 200);
pub const TACTIC_BORDER: Color = Color::DarkGray;
pub const GOAL_FG: Color = Color::Rgb(180, 100, 100);
pub const COMPLETED_FG: Color = Color::Green;
pub const CURRENT_BORDER: Color = Color::Cyan;
pub const INCOMPLETE_BORDER: Color = Color::Yellow;
pub const COMPLETED_BORDER: Color = Color::Green;
