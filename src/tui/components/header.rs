//! Header displaying file and cursor position.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::Component;
use crate::tui_ipc::CursorInfo;

#[derive(Default)]
pub struct Header {
    cursor: Option<CursorInfo>,
}

impl Component for Header {
    type Input = Option<CursorInfo>;
    type Event = ();

    fn update(&mut self, input: Self::Input) {
        self.cursor = input;
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
}
