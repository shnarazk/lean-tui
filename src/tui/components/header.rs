//! Header component displaying file and cursor position.

use crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::Component;
use crate::{tui::app::ClickRegion, tui_ipc::CursorInfo};

/// Header component showing file name and cursor position.
pub struct Header {
    cursor: Option<CursorInfo>,
}

impl Header {
    pub fn new() -> Self {
        Self { cursor: None }
    }

    pub fn set_cursor(&mut self, cursor: Option<CursorInfo>) {
        self.cursor = cursor;
    }
}

impl Component for Header {
    fn handle_event(&mut self, _event: &Event) -> bool {
        false // Header doesn't handle events
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let Some(cursor) = &self.cursor else {
            let waiting =
                Paragraph::new("Waiting for cursor...").style(Style::new().fg(Color::DarkGray));
            frame.render_widget(waiting, area);
            return;
        };

        let filename = cursor.filename().unwrap_or("?");
        let position = format!(
            "{}:{}",
            cursor.position.line + 1,
            cursor.position.character + 1
        );

        let file_width = u16::try_from(6 + filename.len()).unwrap_or(u16::MAX);
        let pos_width = u16::try_from(6 + position.len()).unwrap_or(u16::MAX);

        let [file_area, pos_area, method_area] = Layout::horizontal([
            Constraint::Length(file_width),
            Constraint::Length(pos_width),
            Constraint::Min(0),
        ])
        .areas(area);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                "File: ".into(),
                Span::styled(filename, Style::new().fg(Color::Green)),
            ])),
            file_area,
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                "Pos: ".into(),
                Span::styled(position, Style::new().fg(Color::Yellow)),
            ])),
            pos_area,
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                "(".into(),
                Span::styled(&cursor.method, Style::new().fg(Color::DarkGray)),
                ")".into(),
            ])),
            method_area,
        );
    }

    fn click_regions(&self) -> &[ClickRegion] {
        &[] // Header has no clickable regions
    }
}
