//! Goal before section - displays the goal state before the current tactic.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::lean_rpc::PaperproofGoalInfo;

/// Render the goal state before the current tactic.
pub fn render_goal_before(frame: &mut Frame, area: Rect, goal_before: &PaperproofGoalInfo) {
    let style = Style::new().fg(Color::DarkGray).add_modifier(Modifier::DIM);
    let label_style = Style::new().fg(Color::Blue).add_modifier(Modifier::DIM);

    let lines = vec![
        Line::from(vec![
            Span::styled("Before: ", label_style),
            Span::styled(&goal_before.type_, style),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}
