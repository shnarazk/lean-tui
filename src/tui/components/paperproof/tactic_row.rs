//! Tactic row component - displays divider between hypotheses and goals.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

/// Render a divider between hypotheses and goals.
#[allow(clippy::cast_possible_truncation)]
pub fn render_divider(frame: &mut Frame, area: Rect, tactic: Option<&str>) {
    let style = Style::new().fg(Color::DarkGray);
    let half_width = area.width.saturating_sub(3) / 2;

    let line = tactic.map_or_else(
        || {
            format!(
                "{}▼{}",
                "─".repeat(half_width as usize),
                "─".repeat(half_width as usize)
            )
        },
        |label| {
            let label_width = label.chars().count().min(20);
            let side_width = area.width.saturating_sub(label_width as u16 + 4) / 2;
            let display = if label.len() > 20 {
                format!("{}...", &label[..17])
            } else {
                label.to_string()
            };
            format!(
                "{}─[{}]─{}",
                "─".repeat(side_width as usize),
                display,
                "─".repeat(side_width as usize)
            )
        },
    );

    frame.render_widget(Paragraph::new(line).style(style).centered(), area);
}
