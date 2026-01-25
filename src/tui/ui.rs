//! UI rendering for the TUI.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::*,
    widgets::{Block, Clear, Paragraph, Wrap},
};

use super::app::{App, ClickRegion, HypothesisFilters, LoadStatus, SelectableItem};
use crate::{
    lean_rpc::{DiffTag, Goal, Hypothesis},
    tui_ipc::SOCKET_PATH,
};

/// Kind of cell in the diff grid, combining filter and interactivity.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CellKind {
    /// Current state: show all items, interactive (click regions)
    Current,
    /// Previous state: exclude inserted items, non-interactive
    Previous,
    /// Next state: exclude removed items, non-interactive
    Next,
}

/// Column layout configuration for diff view.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ColumnLayout {
    /// Single column (Current only)
    Single,
    /// Two columns: Previous and Current
    PreviousAndCurrent,
    /// Two columns: Current and Next
    CurrentAndNext,
    /// Three columns: Previous, Current, Next
    All,
}

/// Render the UI.
pub fn render(frame: &mut Frame, app: &mut App) {
    let [main_area, help_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    render_main(frame, app, main_area);
    render_status_bar(frame, help_area, app.filters);

    // Render help popup on top if visible
    if app.show_help {
        render_help_popup(frame);
    }
}

/// Render the main content area.
fn render_main(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered()
        .title(" lean-tui ")
        .border_style(Style::new().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if !app.connected {
        frame.render_widget(
            Paragraph::new(format!("Connecting to {SOCKET_PATH}...")),
            inner,
        );
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
        "File: ".into(),
        Span::styled(app.cursor.filename(), Style::new().fg(Color::Green)),
        "  Pos: ".into(),
        Span::styled(
            format!("{}:{}", app.cursor.line() + 1, app.cursor.character() + 1),
            Style::new().fg(Color::Yellow),
        ),
        "  (".into(),
        Span::styled(&app.cursor.method, Style::new().fg(Color::DarkGray)),
        ")".into(),
    ]);
    frame.render_widget(Paragraph::new(header), area);
}

/// Render goals with adaptive grid layout.
/// Rows = goals (subproofs), Columns = temporal states (Previous/Current/Next).
fn render_goals(frame: &mut Frame, app: &mut App, area: Rect) {
    app.click_regions.clear();

    if app.goals().is_empty() {
        frame.render_widget(
            Paragraph::new("No goals").style(Style::new().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    // Determine column layout based on user toggles, diffs, and terminal width
    let layout = determine_column_layout(app, area.width);

    if layout == ColumnLayout::Single {
        render_goals_single_column(frame, app, area);
        return;
    }

    // Create header row + one row per goal
    let mut row_constraints = vec![Constraint::Length(1)]; // Header row
    for goal in app.goals() {
        // Each goal needs: header + hypotheses + target + blank line
        let height = 1 + goal.hyps.len() + 1 + 1;
        #[allow(clippy::cast_possible_truncation)]
        row_constraints.push(Constraint::Length(height as u16));
    }

    let rows = Layout::vertical(row_constraints).split(area);

    // Render header row
    render_grid_header(frame, rows[0], layout);

    // Render each goal as a row (use index to avoid borrow conflict)
    let selection = app.current_selection();
    let num_goals = app.goals().len();
    for goal_idx in 0..num_goals {
        render_goal_row(
            frame,
            app,
            rows[goal_idx + 1],
            goal_idx,
            selection.as_ref(),
            layout,
        );
    }
}

/// Determine the column layout based on app state and terminal width.
const fn determine_column_layout(app: &App, width: u16) -> ColumnLayout {
    // Check if we have temporal columns enabled and available
    let has_previous = app.columns.previous && app.temporal_goals.previous.is_some();
    let has_next = app.columns.next && app.temporal_goals.next.is_some();

    if width < 80 || (!has_previous && !has_next) {
        return ColumnLayout::Single;
    }

    match (has_previous, has_next, width >= 120) {
        (true, true, true) => ColumnLayout::All,
        (true, _, _) => ColumnLayout::PreviousAndCurrent,
        (false, true, _) => ColumnLayout::CurrentAndNext,
        (false, false, _) => ColumnLayout::Single,
    }
}

/// Column spacing for the grid layout.
const COLUMN_SPACING: u16 = 2;

/// Render the grid header row with column titles.
fn render_grid_header(frame: &mut Frame, area: Rect, layout: ColumnLayout) {
    match layout {
        ColumnLayout::Single => {}
        ColumnLayout::All => {
            let [prev, curr, next] = Layout::horizontal([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .spacing(COLUMN_SPACING)
            .areas(area);

            frame.render_widget(header_label("Previous", false), prev);
            frame.render_widget(header_label("Current", true), curr);
            frame.render_widget(header_label("Next", false), next);
        }
        ColumnLayout::PreviousAndCurrent | ColumnLayout::CurrentAndNext => {
            let [left, right] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .spacing(COLUMN_SPACING)
                    .areas(area);

            let (left_label, right_label) = match layout {
                ColumnLayout::PreviousAndCurrent => ("Previous", "Current"),
                ColumnLayout::CurrentAndNext => ("Current", "Next"),
                _ => unreachable!(),
            };

            frame.render_widget(
                header_label(left_label, layout == ColumnLayout::CurrentAndNext),
                left,
            );
            frame.render_widget(
                header_label(right_label, layout == ColumnLayout::PreviousAndCurrent),
                right,
            );
        }
    }
}

/// Create a centered header label.
fn header_label(text: &str, is_current: bool) -> Paragraph<'_> {
    let style = if is_current {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::DarkGray)
    };
    Paragraph::new(text).style(style).centered()
}

/// Render a single goal as a row with columns for each temporal state.
fn render_goal_row(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    goal_idx: usize,
    selection: Option<&SelectableItem>,
    layout: ColumnLayout,
) {
    match layout {
        ColumnLayout::Single => {}
        ColumnLayout::All => {
            let [prev, curr, next] = Layout::horizontal([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .spacing(COLUMN_SPACING)
            .areas(area);

            render_goal_cell(frame, app, prev, goal_idx, CellKind::Previous);
            render_goal_cell_interactive(frame, app, curr, goal_idx, selection);
            render_goal_cell(frame, app, next, goal_idx, CellKind::Next);
        }
        ColumnLayout::PreviousAndCurrent | ColumnLayout::CurrentAndNext => {
            let [left, right] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .spacing(COLUMN_SPACING)
                    .areas(area);

            match layout {
                ColumnLayout::PreviousAndCurrent => {
                    render_goal_cell(frame, app, left, goal_idx, CellKind::Previous);
                    render_goal_cell_interactive(frame, app, right, goal_idx, selection);
                }
                ColumnLayout::CurrentAndNext => {
                    render_goal_cell_interactive(frame, app, left, goal_idx, selection);
                    render_goal_cell(frame, app, right, goal_idx, CellKind::Next);
                }
                _ => unreachable!(),
            }
        }
    }
}

/// Render a non-interactive goal cell (Previous or Next column).
fn render_goal_cell(frame: &mut Frame, app: &App, area: Rect, goal_idx: usize, kind: CellKind) {
    // Get the goal state for this temporal slot
    let goal_state = match kind {
        CellKind::Previous => app.temporal_goals.previous.as_ref(),
        CellKind::Next => app.temporal_goals.next.as_ref(),
        CellKind::Current => Some(&app.temporal_goals.current),
    };

    let Some(state) = goal_state else {
        frame.render_widget(Paragraph::new("—").fg(Color::DarkGray).centered(), area);
        return;
    };

    // Handle loading/error/not available states
    match &state.status {
        LoadStatus::Loading => {
            frame.render_widget(
                Paragraph::new("Loading...").fg(Color::DarkGray).centered(),
                area,
            );
            return;
        }
        LoadStatus::NotAvailable => {
            let msg = match kind {
                CellKind::Previous => "Start of proof",
                CellKind::Next => "End of proof",
                CellKind::Current => "No goals",
            };
            frame.render_widget(Paragraph::new(msg).fg(Color::DarkGray).centered(), area);
            return;
        }
        LoadStatus::Error(e) => {
            frame.render_widget(
                Paragraph::new(format!("Error: {e}"))
                    .fg(Color::Red)
                    .centered(),
                area,
            );
            return;
        }
        LoadStatus::Ready => {}
    }

    // Get the goal from this temporal slot's goals
    let Some(goal) = state.goals.get(goal_idx) else {
        frame.render_widget(Paragraph::new("—").fg(Color::DarkGray).centered(), area);
        return;
    };

    // For temporal columns, use CellKind::Current to show their own diff markers
    // (the goals from Lean already have diff info relative to their position)
    let render_kind = CellKind::Current;
    let lines = build_goal_lines(goal, goal_idx, None, render_kind, app.filters);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Register a click region for a line at a given offset within an area.
fn register_click_region(
    click_regions: &mut Vec<ClickRegion>,
    area: Rect,
    line_offset: u16,
    item: SelectableItem,
) {
    let line_y = area.y + line_offset;
    if line_y < area.y + area.height {
        click_regions.push(ClickRegion {
            area: Rect::new(area.x, line_y, area.width, 1),
            item,
        });
    }
}

/// Render an interactive goal cell (Current column) with click regions.
fn render_goal_cell_interactive(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    goal_idx: usize,
    selection: Option<&SelectableItem>,
) {
    let Some(goal) = app.goals().get(goal_idx).cloned() else {
        return;
    };
    let lines = build_goal_lines(&goal, goal_idx, selection, CellKind::Current, app.filters);

    // Register click regions (header at line 0, then hypotheses, then target)
    // Must match the order used in build_goal_lines
    let mut line_offset = 1u16;

    let hyp_indices: Vec<usize> = if app.filters.reverse_order {
        (0..goal.hyps.len()).rev().collect()
    } else {
        (0..goal.hyps.len()).collect()
    };

    for hyp_idx in hyp_indices {
        let hyp = &goal.hyps[hyp_idx];
        if app.filters.should_show(hyp) {
            register_click_region(
                &mut app.click_regions,
                area,
                line_offset,
                SelectableItem::Hypothesis { goal_idx, hyp_idx },
            );
            line_offset += 1;
        }
    }
    register_click_region(
        &mut app.click_regions,
        area,
        line_offset,
        SelectableItem::GoalTarget { goal_idx },
    );

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Build the lines for a goal cell.
fn build_goal_lines(
    goal: &Goal,
    goal_idx: usize,
    selection: Option<&SelectableItem>,
    kind: CellKind,
    filters: HypothesisFilters,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(
        Line::from(goal_header(goal, goal_idx))
            .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    );

    // Build hypothesis indices, respecting reverse order
    let hyp_indices: Vec<usize> = if filters.reverse_order {
        (0..goal.hyps.len()).rev().collect()
    } else {
        (0..goal.hyps.len()).collect()
    };

    for hyp_idx in hyp_indices {
        let hyp = &goal.hyps[hyp_idx];
        if should_skip_hyp(hyp, kind) {
            continue;
        }
        if !filters.should_show(hyp) {
            continue;
        }
        let is_selected = selection == Some(&SelectableItem::Hypothesis { goal_idx, hyp_idx });
        lines.push(render_hypothesis_line(hyp, is_selected, kind, filters));
    }

    let is_target_selected = selection == Some(&SelectableItem::GoalTarget { goal_idx });
    lines.push(render_target_line(goal, is_target_selected, kind));

    lines
}

/// Render single column with inline diff markers (narrow terminal).
fn render_goals_single_column(frame: &mut Frame, app: &mut App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    let mut click_items: Vec<Option<SelectableItem>> = Vec::new();

    if let Some(error) = &app.error {
        lines.push(Line::from(format!("Error: {error}")).fg(Color::Red));
        click_items.push(None);
        lines.push(Line::default());
        click_items.push(None);
    }

    let selection = app.current_selection();
    for (goal_idx, goal) in app.goals().iter().enumerate() {
        // Goal header (not clickable)
        lines.push(
            Line::from(goal_header(goal, goal_idx))
                .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        );
        click_items.push(None);

        // Build hypothesis indices, respecting reverse order
        let hyp_indices: Vec<usize> = if app.filters.reverse_order {
            (0..goal.hyps.len()).rev().collect()
        } else {
            (0..goal.hyps.len()).collect()
        };

        // Hypotheses (clickable, filtered)
        for hyp_idx in hyp_indices {
            let hyp = &goal.hyps[hyp_idx];
            if !app.filters.should_show(hyp) {
                continue;
            }
            let is_selected = selection == Some(SelectableItem::Hypothesis { goal_idx, hyp_idx });
            lines.push(render_hypothesis_line(
                hyp,
                is_selected,
                CellKind::Current,
                app.filters,
            ));
            click_items.push(Some(SelectableItem::Hypothesis { goal_idx, hyp_idx }));
        }

        // Goal target (clickable)
        let is_target_selected = selection == Some(SelectableItem::GoalTarget { goal_idx });
        lines.push(render_target_line(
            goal,
            is_target_selected,
            CellKind::Current,
        ));
        click_items.push(Some(SelectableItem::GoalTarget { goal_idx }));

        // Empty line between goals
        lines.push(Line::default());
        click_items.push(None);
    }

    // Record click regions
    for (line_idx, item) in click_items.into_iter().enumerate() {
        if let Some(selectable) = item {
            #[allow(clippy::cast_possible_truncation)]
            register_click_region(&mut app.click_regions, area, line_idx as u16, selectable);
        }
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Check if a hypothesis should be skipped based on cell kind.
const fn should_skip_hyp(hyp: &Hypothesis, kind: CellKind) -> bool {
    match kind {
        CellKind::Current => false,
        CellKind::Previous => hyp.is_inserted,
        CellKind::Next => hyp.is_removed,
    }
}

/// Build goal header string.
fn goal_header(goal: &Goal, goal_idx: usize) -> String {
    match goal.user_name.as_deref() {
        Some("Expected") => "Expected:".to_string(),
        Some(name) => format!("case {name}:"),
        None => format!("Goal {}:", goal_idx + 1),
    }
}

/// Render hypothesis line with cell-aware styling.
fn render_hypothesis_line(
    hyp: &Hypothesis,
    is_selected: bool,
    kind: CellKind,
    filters: HypothesisFilters,
) -> Line<'static> {
    let names = hyp.names.join(", ");
    let prefix = selection_prefix(is_selected);

    // Build type string, optionally including let-value
    let type_str = if filters.hide_let_values {
        hyp.type_.clone()
    } else {
        hyp.val.as_ref().map_or_else(
            || hyp.type_.clone(),
            |val| format!("{} := {val}", hyp.type_),
        )
    };

    // In non-Current columns, don't show diff markers (items are already filtered)
    let (style, diff_marker) = match kind {
        CellKind::Previous | CellKind::Next => (item_style(is_selected, Color::White), ""),
        CellKind::Current if hyp.is_inserted => (item_style(is_selected, Color::Green), " [+]"),
        CellKind::Current if hyp.is_removed => (
            item_style(is_selected, Color::Red).add_modifier(Modifier::CROSSED_OUT),
            " [-]",
        ),
        CellKind::Current => match hyp.diff_status {
            Some(DiffTag::WasChanged) => (item_style(is_selected, Color::Yellow), " [~]"),
            Some(DiffTag::WasInserted) => (item_style(is_selected, Color::Green), " [+]"),
            Some(DiffTag::WasDeleted) => (item_style(is_selected, Color::Red), " [-]"),
            Some(DiffTag::WillChange | DiffTag::WillInsert | DiffTag::WillDelete) | None => {
                (item_style(is_selected, Color::White), "")
            }
        },
    };

    Line::from(format!("{prefix}{names} : {type_str}{diff_marker}")).style(style)
}

/// Render goal target line with cell-aware styling.
fn render_target_line(goal: &Goal, is_selected: bool, kind: CellKind) -> Line<'static> {
    let prefix = selection_prefix(is_selected);

    let (style, diff_marker) = match kind {
        CellKind::Current if goal.is_inserted => (item_style(is_selected, Color::Green), " [+]"),
        CellKind::Current if goal.is_removed => (
            item_style(is_selected, Color::Red).add_modifier(Modifier::CROSSED_OUT),
            " [-]",
        ),
        _ => (item_style(is_selected, Color::Cyan), ""),
    };

    Line::from(format!(
        "{prefix}{}{}{diff_marker}",
        goal.prefix, goal.target
    ))
    .style(style)
}
/// Style for items with optional selection highlighting.
const fn item_style(is_selected: bool, fg_color: Color) -> Style {
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

/// Render the status bar at the bottom.
fn render_status_bar(frame: &mut Frame, area: Rect, filters: HypothesisFilters) {
    // Build filter indicator string
    let mut filter_chars = Vec::new();
    if filters.hide_instances {
        filter_chars.push('i');
    }
    if filters.hide_types {
        filter_chars.push('t');
    }
    if filters.hide_inaccessible {
        filter_chars.push('a');
    }
    if filters.hide_let_values {
        filter_chars.push('l');
    }
    if filters.reverse_order {
        filter_chars.push('r');
    }

    let filter_status: String = if filter_chars.is_empty() {
        String::new()
    } else {
        format!(" [{}]", filter_chars.into_iter().collect::<String>())
    };

    let status = Line::from(vec![
        Span::styled("?", Style::new().fg(Color::Cyan)),
        ": help  ".into(),
        Span::styled("j/k", Style::new().fg(Color::Cyan)),
        ": nav  ".into(),
        Span::styled("Enter", Style::new().fg(Color::Cyan)),
        ": go".into(),
        Span::styled(filter_status, Style::new().fg(Color::Green)),
        "  ".into(),
        Span::styled("q", Style::new().fg(Color::Cyan)),
        ": quit".into(),
    ]);
    frame.render_widget(Paragraph::new(status), area);
}

/// Center a rect within another rect.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

/// Render the help popup overlay.
fn render_help_popup(frame: &mut Frame) {
    let area = centered_rect(60, 70, frame.area());

    // Clear background
    frame.render_widget(Clear, area);

    let block = Block::bordered()
        .title(" Keyboard Shortcuts ")
        .title_bottom(" Press ? or Esc to close ")
        .border_style(Style::new().fg(Color::Cyan));

    let help_text = vec![
        Line::from(vec![Span::styled(
            "Navigation",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  j / ↓    ", Style::new().fg(Color::Cyan)),
            "Move selection down".into(),
        ]),
        Line::from(vec![
            Span::styled("  k / ↑    ", Style::new().fg(Color::Cyan)),
            "Move selection up".into(),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::new().fg(Color::Cyan)),
            "Go to definition".into(),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Hypothesis Filters",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  i        ", Style::new().fg(Color::Cyan)),
            "Toggle hide instances".into(),
        ]),
        Line::from(vec![
            Span::styled("  t        ", Style::new().fg(Color::Cyan)),
            "Toggle hide types".into(),
        ]),
        Line::from(vec![
            Span::styled("  a        ", Style::new().fg(Color::Cyan)),
            "Toggle hide inaccessible".into(),
        ]),
        Line::from(vec![
            Span::styled("  l        ", Style::new().fg(Color::Cyan)),
            "Toggle show let := values".into(),
        ]),
        Line::from(vec![
            Span::styled("  r        ", Style::new().fg(Color::Cyan)),
            "Reverse hypothesis order".into(),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Diff View",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  p        ", Style::new().fg(Color::Cyan)),
            "Toggle Previous column".into(),
        ]),
        Line::from(vec![
            Span::styled("  n        ", Style::new().fg(Color::Cyan)),
            "Toggle Next column".into(),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "General",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ?        ", Style::new().fg(Color::Cyan)),
            "Toggle this help".into(),
        ]),
        Line::from(vec![
            Span::styled("  q        ", Style::new().fg(Color::Cyan)),
            "Quit".into(),
        ]),
    ];

    let paragraph = Paragraph::new(help_text).block(block);
    frame.render_widget(paragraph, area);
}
