//! Welcome screen shown when no goals are available.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Block, Paragraph, Widget},
};

/// Welcome screen widget.
pub struct WelcomeScreen;

impl Widget for WelcomeScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(" lean-tui ")
            .title_alignment(Alignment::Center)
            .border_style(Style::new().fg(Color::Cyan));

        let inner = block.inner(area);
        block.render(area, buf);

        let text = "Standalone TUI infoview for Lean 4 theorem prover\n\nMove cursor to a tactic \
                    in your editor.";

        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::new().fg(Color::DarkGray))
            .render(inner, buf);
    }
}
