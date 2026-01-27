//! Tactic row component - displays divider between hypotheses and goals.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

/// A horizontal divider with an optional centered label.
pub struct Divider<'a> {
    label: Option<&'a str>,
    style: Style,
}

impl<'a> Divider<'a> {
    pub const fn new() -> Self {
        Self {
            label: None,
            style: Style::new(),
        }
    }

    #[allow(dead_code)]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Default for Divider<'_> {
    fn default() -> Self {
        Self::new().style(Style::new().fg(Color::DarkGray))
    }
}

impl Widget for Divider<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let line = self.label.map_or_else(
            || build_simple_divider(area.width),
            |label| build_labeled_divider(area.width, label),
        );

        buf.set_string(area.x, area.y, &line, self.style);
    }
}

fn build_simple_divider(width: u16) -> String {
    let half = width.saturating_sub(1) / 2;
    let remainder = width.saturating_sub(1) % 2;
    format!(
        "{}▼{}",
        "─".repeat(half as usize),
        "─".repeat((half + remainder) as usize)
    )
}

fn build_labeled_divider(width: u16, label: &str) -> String {
    let max_label_len = 20;
    let display = if label.chars().count() > max_label_len {
        format!(
            "{}…",
            label.chars().take(max_label_len - 1).collect::<String>()
        )
    } else {
        label.to_string()
    };

    #[allow(clippy::cast_possible_truncation)]
    let label_width = display.chars().count() as u16;
    let brackets_width = 4; // "─[" + "]─"
    let available = width.saturating_sub(label_width + brackets_width);
    let left = available / 2;
    let right = available - left;

    format!(
        "{}─[{}]─{}",
        "─".repeat(left as usize),
        display,
        "─".repeat(right as usize)
    )
}

/// Convenience function for simple divider rendering.
pub fn divider() -> Divider<'static> {
    Divider::default()
}
