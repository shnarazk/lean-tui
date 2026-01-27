//! Welcome screen shown when no goals are available.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Cell, Paragraph, Row, Table, Widget},
};

/// Welcome screen widget.
pub struct WelcomeScreen;

impl Widget for WelcomeScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(" Welcome to Lean-TUI ")
            .title_alignment(Alignment::Center)
            .border_style(Style::new().fg(Color::Cyan));

        let inner = block.inner(area);
        block.render(area, buf);

        // Layout: header text, spacing, keybindings table
        let content_height = 12u16;
        let vertical_padding = inner.height.saturating_sub(content_height) / 2;

        let [_, content_area, _] = Layout::vertical([
            Constraint::Length(vertical_padding),
            Constraint::Min(content_height),
            Constraint::Fill(1),
        ])
        .areas(inner);

        let [header_area, _, table_area] = Layout::vertical([
            Constraint::Length(6),
            Constraint::Length(1),
            Constraint::Min(6),
        ])
        .areas(content_area);

        // Render header
        let header = build_header();
        Paragraph::new(header)
            .alignment(Alignment::Center)
            .render(header_area, buf);

        // Render keybindings table (centered horizontally)
        let table = build_keybindings_table();
        let table_width = 38u16;
        let [table_centered] = Layout::horizontal([Constraint::Length(table_width)])
            .flex(Flex::Center)
            .areas(table_area);

        table.render(table_centered, buf);
    }
}

fn build_header() -> Text<'static> {
    let title_style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim_style = Style::new().fg(Color::DarkGray);

    Text::from(vec![
        Line::from(Span::styled(
            "Interactive proof explorer for Lean 4",
            title_style,
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Move your cursor to a tactic in your editor",
            dim_style,
        )),
        Line::from(Span::styled("to see the proof state here.", dim_style)),
        Line::from(""),
        Line::from(Span::styled("Keybindings", title_style)),
    ])
}

fn build_keybindings_table() -> Table<'static> {
    let key_style = Style::new().fg(Color::Yellow);
    let desc_style = Style::new().fg(Color::White);

    let keybindings = [
        ("j/k", "Navigate up/down"),
        ("d/Enter", "Go to definition"),
        ("t", "Go to type definition"),
        ("]/[", "Next/previous mode"),
        ("?", "Show full help"),
        ("q", "Quit"),
    ];

    let rows: Vec<Row> = keybindings
        .into_iter()
        .map(|(key, desc)| {
            Row::new([
                Cell::from(key).style(key_style),
                Cell::from(desc).style(desc_style),
            ])
        })
        .collect();

    Table::new(rows, [Constraint::Length(10), Constraint::Fill(1)])
}
