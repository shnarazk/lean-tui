//! Color palette for the proof tree view, matching Paperproof's visual style.

use ratatui::style::Color;

// Proof hypothesis colors (green tones)
pub const HYPOTHESIS_BG: Color = Color::Rgb(30, 45, 30);
pub const HYPOTHESIS_FG: Color = Color::Rgb(150, 200, 150);

// Data hypothesis colors (yellow/tan tones)
pub const DATA_HYP_BG: Color = Color::Rgb(50, 45, 25);
pub const DATA_HYP_FG: Color = Color::Rgb(200, 180, 120);

pub const TACTIC_BORDER: Color = Color::DarkGray;

// Goal color
pub const GOAL_FG: Color = Color::Rgb(200, 140, 140);
pub const COMPLETED_FG: Color = Color::Green;
pub const CURRENT_BORDER: Color = Color::Cyan;
pub const INCOMPLETE_BORDER: Color = Color::Yellow;
pub const COMPLETED_BORDER: Color = Color::Green;
