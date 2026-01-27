//! Common render helper functions for modes.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::Stylize,
    style::Color,
    widgets::Paragraph,
    Frame,
};

/// Render an error message at the top of the area and return the remaining
/// area. Returns the original area if there's no error.
pub fn render_error(frame: &mut Frame, area: Rect, error: Option<&str>) -> Rect {
    let Some(error) = error else {
        return area;
    };

    let [error_area, rest] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(area);
    frame.render_widget(
        Paragraph::new(format!("Error: {error}")).fg(Color::Red),
        error_area,
    );
    rest
}

/// Render a "No goals" placeholder when goals are empty.
pub fn render_no_goals(frame: &mut Frame, area: Rect) {
    frame.render_widget(Paragraph::new("No goals").fg(Color::DarkGray), area);
}
