//! HelpMenu component - overlay showing keyboard shortcuts.

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use super::Component;
use crate::tui::app::ClickRegion;

const KEYBINDINGS: &[(&str, &str)] = &[
    ("j/k", "navigate"),
    ("Enter", "go to definition"),
    ("d", "toggle definition"),
    ("i", "toggle instances"),
    ("t", "toggle types"),
    ("a", "toggle inaccessible"),
    ("l", "toggle let values"),
    ("r", "toggle reverse order"),
    ("p", "previous column"),
    ("n", "next column"),
    ("?", "close help"),
    ("q", "quit"),
];

/// Help menu popup showing keyboard shortcuts.
pub struct HelpMenu {
    visible: bool,
}

impl HelpMenu {
    pub fn new() -> Self {
        Self { visible: false }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
}

impl Component for HelpMenu {
    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }

        let Event::Key(key) = event else {
            return false;
        };
        if key.kind != KeyEventKind::Press {
            return false;
        }

        match key.code {
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

    fn click_regions(&self) -> &[ClickRegion] {
        &[]
    }
}

impl Default for HelpMenu {
    fn default() -> Self {
        Self::new()
    }
}
