//! UI rendering for the TUI.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use super::app::{App, ClickRegion, SelectableItem};
use crate::{lean_rpc::DiffTag, tui_ipc::SOCKET_PATH};

/// Render the UI.
pub fn render(frame: &mut Frame, app: &mut App) {
    let [main_area, help_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    render_main(frame, app, main_area);
    render_help(frame, help_area);
}

/// Render the main content area.
fn render_main(frame: &mut Frame, app: &mut App, area: Rect) {
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
fn render_goals(frame: &mut Frame, app: &mut App, area: Rect) {
    // Clear previous click regions
    app.click_regions.clear();

    let mut lines: Vec<Line> = Vec::new();
    let mut click_items: Vec<Option<SelectableItem>> = Vec::new();

    if let Some(error) = &app.error {
        lines.push(Line::from(format!("Error: {error}")).style(Style::default().fg(Color::Red)));
        click_items.push(None);
        lines.push(Line::from(""));
        click_items.push(None);
    }

    if app.goals.is_empty() {
        lines.push(Line::from("No goals").style(Style::default().fg(Color::DarkGray)));
        click_items.push(None);
    } else {
        let selection = app.current_selection();
        for (goal_idx, goal) in app.goals.iter().enumerate() {
            render_goal(
                &mut lines,
                &mut click_items,
                goal,
                goal_idx,
                selection.as_ref(),
            );
        }
    }

    // Record click regions based on line positions
    for (line_idx, item) in click_items.into_iter().enumerate() {
        if let Some(selectable) = item {
            let line_y = area.y + line_idx as u16;
            if line_y < area.y + area.height {
                app.click_regions.push(ClickRegion {
                    area: Rect::new(area.x, line_y, area.width, 1),
                    item: selectable,
                });
            }
        }
    }

    let content = Paragraph::new(Text::from(lines));
    frame.render_widget(content, area);
}

fn render_goal(
    lines: &mut Vec<Line<'_>>,
    click_items: &mut Vec<Option<SelectableItem>>,
    goal: &crate::lean_rpc::Goal,
    goal_idx: usize,
    selection: Option<&SelectableItem>,
) {
    // Goal header (not clickable)
    lines.push(
        Line::from(format!("Goal {}:", goal_idx + 1)).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    );
    click_items.push(None);

    // Hypotheses (clickable)
    for (hyp_idx, hyp) in goal.hyps.iter().enumerate() {
        let is_selected = selection == Some(&SelectableItem::Hypothesis { goal_idx, hyp_idx });
        let line = render_hypothesis_line(hyp, is_selected);
        lines.push(line);
        click_items.push(Some(SelectableItem::Hypothesis { goal_idx, hyp_idx }));
    }

    // Goal target (clickable)
    let is_target_selected = selection == Some(&SelectableItem::GoalTarget { goal_idx });
    let prefix = selection_prefix(is_target_selected);

    // Determine style and marker based on goal diff status
    let (target_style, diff_marker) = if goal.is_inserted {
        (diff_style(is_target_selected, Color::Green), " [+]")
    } else if goal.is_removed {
        (
            diff_style(is_target_selected, Color::Red).add_modifier(Modifier::CROSSED_OUT),
            " [-]",
        )
    } else {
        (selected_style(is_target_selected, Color::Cyan), "")
    };

    lines.push(Line::from(format!("{prefix}⊢ {}{diff_marker}", goal.target)).style(target_style));
    click_items.push(Some(SelectableItem::GoalTarget { goal_idx }));

    // Empty line (not clickable)
    lines.push(Line::from(""));
    click_items.push(None);
}

fn render_hypothesis_line(hyp: &crate::lean_rpc::Hypothesis, is_selected: bool) -> Line<'static> {
    let names = hyp.names.join(", ");
    let prefix = selection_prefix(is_selected);

    // Determine style and marker based on diff status
    let (style, diff_marker) = if hyp.is_inserted {
        (diff_style(is_selected, Color::Green), " [+]")
    } else if hyp.is_removed {
        (
            diff_style(is_selected, Color::Red).add_modifier(Modifier::CROSSED_OUT),
            " [-]",
        )
    } else {
        match hyp.diff_status {
            Some(DiffTag::WasChanged | DiffTag::WillChange) => {
                (diff_style(is_selected, Color::Yellow), " [~]")
            }
            Some(DiffTag::WasInserted | DiffTag::WillInsert) => {
                (diff_style(is_selected, Color::Green), " [+]")
            }
            Some(DiffTag::WasDeleted | DiffTag::WillDelete) => {
                (diff_style(is_selected, Color::Red), " [-]")
            }
            None => (selected_style(is_selected, Color::White), ""),
        }
    };

    Line::from(format!("{prefix}{names} : {}{diff_marker}", hyp.type_)).style(style)
}

/// Style for diff-highlighted items (with selection support).
const fn diff_style(is_selected: bool, fg_color: Color) -> Style {
    if is_selected {
        Style::new().bg(Color::DarkGray).fg(fg_color)
    } else {
        Style::new().fg(fg_color)
    }
}

const fn selected_style(is_selected: bool, fg_color: Color) -> Style {
    if is_selected {
        Style::new().bg(Color::DarkGray).fg(fg_color)
    } else {
        Style::new().fg(fg_color)
    }
}

const fn selection_prefix(is_selected: bool) -> &'static str {
    if is_selected {
        "▶ "
    } else {
        "  "
    }
}

/// Render the help bar at the bottom.
fn render_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled("j/k", Style::default().fg(Color::Cyan)),
        Span::raw(": navigate  "),
        Span::styled("Enter/Click", Style::default().fg(Color::Cyan)),
        Span::raw(": go to  "),
        Span::styled("q", Style::default().fg(Color::Cyan)),
        Span::raw(": quit"),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}
