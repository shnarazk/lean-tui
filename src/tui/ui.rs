//! UI rendering for the TUI.

use ratatui::{
    layout::{Constraint, Layout},
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use super::app::{App, SelectableItem};
use crate::tui_ipc::SOCKET_PATH;

/// Render the UI.
pub fn render(frame: &mut Frame, app: &App) {
    let [main_area, help_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    render_main(frame, app, main_area);
    render_help(frame, help_area);
}

/// Render the main content area.
fn render_main(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" lean-tui ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if !app.connected {
        let text = Paragraph::new(format!("Connecting to {SOCKET_PATH}..."));
        frame.render_widget(text, inner);
        return;
    }

    let [header_area, content_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(inner);

    render_header(frame, app, header_area);
    render_goals(frame, app, content_area);
}

/// Render the header with cursor info.
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let header = Line::from(vec![
        Span::raw("File: "),
        Span::styled(app.cursor.filename(), Style::default().fg(Color::Green)),
        Span::raw("  Pos: "),
        Span::styled(
            format!("{}:{}", app.cursor.line() + 1, app.cursor.character() + 1),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  ("),
        Span::styled(&app.cursor.method, Style::default().fg(Color::DarkGray)),
        Span::raw(")"),
    ]);
    frame.render_widget(Paragraph::new(header), area);
}

/// Render goals and hypotheses.
fn render_goals(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(error) = &app.error {
        lines.push(Line::from(format!("Error: {error}")).style(Style::default().fg(Color::Red)));
        lines.push(Line::from(""));
    }

    if app.goals.is_empty() {
        lines.push(Line::from("No goals").style(Style::default().fg(Color::DarkGray)));
    } else {
        let selection = app.current_selection();
        for (goal_idx, goal) in app.goals.iter().enumerate() {
            render_goal(&mut lines, goal, goal_idx, selection.as_ref());
        }
    }

    let content = Paragraph::new(Text::from(lines));
    frame.render_widget(content, area);
}

fn render_goal(lines: &mut Vec<Line<'_>>, goal: &crate::lean_rpc::Goal, goal_idx: usize, selection: Option<&SelectableItem>) {
    // Goal header
    lines.push(
        Line::from(format!("Goal {}:", goal_idx + 1)).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    );

    // Hypotheses
    for (hyp_idx, hyp) in goal.hyps.iter().enumerate() {
        let is_selected = selection == Some(&SelectableItem::Hypothesis { goal_idx, hyp_idx });
        let line = render_hypothesis_line(hyp, is_selected);
        lines.push(line);
    }

    // Goal target
    let is_target_selected = selection == Some(&SelectableItem::GoalTarget { goal_idx });
    let target_style = selected_style(is_target_selected, Color::Cyan);
    let prefix = selection_prefix(is_target_selected);
    lines.push(Line::from(format!("{prefix}⊢ {}", goal.target)).style(target_style));

    lines.push(Line::from(""));
}

fn render_hypothesis_line(hyp: &crate::lean_rpc::Hypothesis, is_selected: bool) -> Line<'static> {
    let names = hyp.names.join(", ");
    let style = selected_style(is_selected, Color::White);
    let prefix = selection_prefix(is_selected);
    Line::from(format!("{prefix}{names} : {}", hyp.type_)).style(style)
}

const fn selected_style(is_selected: bool, fg_color: Color) -> Style {
    if is_selected {
        Style::new().bg(Color::DarkGray).fg(fg_color)
    } else {
        Style::new().fg(fg_color)
    }
}

const fn selection_prefix(is_selected: bool) -> &'static str {
    if is_selected { "▶ " } else { "  " }
}

/// Render the help bar at the bottom.
fn render_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled("j/k", Style::default().fg(Color::Cyan)),
        Span::raw(": navigate  "),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::raw(": go to  "),
        Span::styled("q", Style::default().fg(Color::Cyan)),
        Span::raw(": quit"),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}
