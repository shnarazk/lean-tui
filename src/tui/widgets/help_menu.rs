//! Help menu overlay.

use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use super::KeyEvent;
use crate::tui::widgets::interactive_widget::InteractiveWidget;

const KEYBINDINGS: &[(&str, &str)] = &[
    // Display modes
    ("[/]", "cycle display mode"),
    ("b", "toggle goal before"),
    // Navigation
    ("j/k", "navigate up/down"),
    ("Enter", "go to definition"),
    ("p/n", "prev/next column"),
    // Filters
    ("d", "toggle header"),
    ("i", "toggle instances"),
    ("t", "toggle types"),
    ("a", "toggle inaccessible"),
    ("l", "toggle let values"),
    ("r", "reverse hyp order"),
    // Other
    ("?", "close help"),
    ("q", "quit"),
];

#[derive(Default)]
pub struct HelpMenu {
    visible: bool,
}

impl HelpMenu {
    pub const fn toggle(&mut self) {
        self.visible = !self.visible;
    }
}

impl InteractiveWidget for HelpMenu {
    type Input = ();
    type Event = KeyEvent;

    fn update(&mut self, _input: Self::Input) {}

    fn handle_event(&mut self, event: Self::Event) -> bool {
        if !self.visible {
            return false;
        }

        match event.code {
            KeyCode::Esc | KeyCode::Char('?') => {
                self.visible = false;
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let width = 28u16;
        #[allow(clippy::cast_possible_truncation)]
        let height = (KEYBINDINGS.len() as u16) + 2;
        let x = area.width.saturating_sub(width + 1);
        let y = area.height.saturating_sub(height + 2);
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let block = Block::bordered()
            .title(" Help ")
            .border_style(Style::new().fg(Color::Cyan));

        let key_style = Style::new().fg(Color::Cyan);
        let help_lines: Vec<Line> = KEYBINDINGS
            .iter()
            .map(|(key, desc)| {
                Line::from(vec![
                    Span::styled(format!("{key:>6}"), key_style),
                    Span::raw(format!("  {desc}")),
                ])
            })
            .collect();

        frame.render_widget(Paragraph::new(help_lines).block(block), popup_area);
    }
}
