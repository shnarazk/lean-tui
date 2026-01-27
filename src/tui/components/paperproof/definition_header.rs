//! Definition header component - displays the current theorem/lemma name.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui_ipc::DefinitionInfo;

/// Render the definition header (theorem/lemma name).
pub fn render_definition_header(frame: &mut Frame, area: Rect, definition: &DefinitionInfo) {
    let header = Line::from(vec![
        Span::styled(&definition.kind, Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(&definition.name, Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);
    frame.render_widget(Paragraph::new(header), area);
}
