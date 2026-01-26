//! UI rendering for the TUI.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::*,
    widgets::{Block, Clear, Paragraph, Wrap},
};

use super::app::{
    hypothesis_indices, App, ClickRegion, HypothesisFilters, LoadStatus, SelectableItem,
};
use crate::{
    lean_rpc::{DiffTag, Goal, Hypothesis},
    tui_ipc::socket_path,
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

/// Column areas for a single row in grid layout.
struct ColumnAreas {
    previous: Option<Rect>,
    current: Rect,
    next: Option<Rect>,
}

/// Compute column areas for a row based on layout.
fn split_row_columns(row: Rect, layout: ColumnLayout) -> ColumnAreas {
    match layout {
        ColumnLayout::Single => ColumnAreas {
            previous: None,
            current: row,
            next: None,
        },
        ColumnLayout::All => {
            let [prev, curr, next] = Layout::horizontal([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .spacing(COLUMN_SPACING)
            .areas(row);
            ColumnAreas {
                previous: Some(prev),
                current: curr,
                next: Some(next),
            }
        }
        ColumnLayout::PreviousAndCurrent => {
            let [left, right] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .spacing(COLUMN_SPACING)
                    .areas(row);
            ColumnAreas {
                previous: Some(left),
                current: right,
                next: None,
            }
        }
        ColumnLayout::CurrentAndNext => {
            let [left, right] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .spacing(COLUMN_SPACING)
                    .areas(row);
            ColumnAreas {
                previous: None,
                current: left,
                next: Some(right),
            }
        }
    }
}

fn compute_row_constraints(goals: &[Goal]) -> Vec<Constraint> {
    std::iter::once(Constraint::Length(1)) // Header row
        .chain(goals.iter().map(|goal| {
            let height = 1 + goal.hyps.len() + 1 + 1;
            #[allow(clippy::cast_possible_truncation)]
            Constraint::Length(height as u16)
        }))
        .collect()
}

/// Column spacing for the grid layout.
const COLUMN_SPACING: u16 = 2;

/// Render the UI.
pub fn render(frame: &mut Frame, app: &App) {
    let [main_area, help_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    render_main(frame, app, main_area);
    render_status_bar(frame, help_area, app.filters);

    // Render help popup on top if visible
    if app.show_help {
        render_help_popup(frame);
    }
}

/// Compute click regions for the current layout.
/// Must use the same layout logic as render to ensure consistency.
pub fn compute_click_regions(app: &App, size: Rect) -> Vec<ClickRegion> {
    let mut regions = Vec::new();

    let [main_area, _help_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(size);

    // Main content layout
    let block_inner = {
        let block = ratatui::widgets::Block::bordered();
        block.inner(main_area)
    };

    if !app.connected || app.goals().is_empty() {
        return regions;
    }

    let [_header_area, content_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(block_inner);

    let layout = determine_column_layout(app, content_area.width);

    if layout == ColumnLayout::Single {
        compute_single_column_regions(app, content_area, &mut regions);
    } else {
        compute_grid_regions(app, content_area, layout, &mut regions);
    }

    regions
}

/// Compute click regions for single column layout.
fn compute_single_column_regions(app: &App, area: Rect, regions: &mut Vec<ClickRegion>) {
    let error_offset: u16 = if app.error.is_some() { 2 } else { 0 };

    for (line_offset, item) in app
        .goals()
        .iter()
        .enumerate()
        .scan(error_offset, |offset, (goal_idx, goal)| {
            *offset += 1; // Goal header

            let hyp_items: Vec<_> = hypothesis_indices(goal.hyps.len(), app.filters.reverse_order)
                .filter(|&hyp_idx| app.filters.should_show(&goal.hyps[hyp_idx]))
                .map(|hyp_idx| {
                    let item_offset = *offset;
                    *offset += 1;
                    (
                        item_offset,
                        SelectableItem::Hypothesis { goal_idx, hyp_idx },
                    )
                })
                .collect();

            let target_offset = *offset;
            *offset += 2; // Target line + empty line

            Some(hyp_items.into_iter().chain(std::iter::once((
                target_offset,
                SelectableItem::GoalTarget { goal_idx },
            ))))
        })
        .flatten()
    {
        register_click_region(regions, area, line_offset, item);
    }
}

/// Compute click regions for grid layout (multi-column).
fn compute_grid_regions(
    app: &App,
    area: Rect,
    layout: ColumnLayout,
    regions: &mut Vec<ClickRegion>,
) {
    let rows = Layout::vertical(compute_row_constraints(app.goals())).split(area);

    app.goals().iter().enumerate().for_each(|(goal_idx, goal)| {
        let cols = split_row_columns(rows[goal_idx + 1], layout);
        register_goal_click_regions(regions, cols.current, goal_idx, goal, app.filters);
    });
}

/// Register click regions for a single goal cell.
fn register_goal_click_regions(
    regions: &mut Vec<ClickRegion>,
    area: Rect,
    goal_idx: usize,
    goal: &Goal,
    filters: HypothesisFilters,
) {
    let hyp_regions = hypothesis_indices(goal.hyps.len(), filters.reverse_order)
        .filter(|&hyp_idx| filters.should_show(&goal.hyps[hyp_idx]))
        .enumerate()
        .map(|(offset, hyp_idx)| {
            #[allow(clippy::cast_possible_truncation)]
            let line_offset = (1 + offset) as u16;
            (
                line_offset,
                SelectableItem::Hypothesis { goal_idx, hyp_idx },
            )
        });

    let hyp_count = hypothesis_indices(goal.hyps.len(), filters.reverse_order)
        .filter(|&hyp_idx| filters.should_show(&goal.hyps[hyp_idx]))
        .count();
    #[allow(clippy::cast_possible_truncation)]
    let target_offset = (1 + hyp_count) as u16;
    let target_region = std::iter::once((target_offset, SelectableItem::GoalTarget { goal_idx }));

    hyp_regions
        .chain(target_region)
        .for_each(|(line_offset, item)| {
            register_click_region(regions, area, line_offset, item);
        });
}

/// Render the main content area.
fn render_main(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered()
        .title(" lean-tui ")
        .border_style(Style::new().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if !app.connected {
        frame.render_widget(
            Paragraph::new(format!("Connecting to {}...", socket_path().display())),
            inner,
        );
        return;
    }

    let [header_area, content_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(inner);

    render_header(frame, app, header_area);
    render_goals(frame, app, content_area);
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let Some(cursor) = &app.cursor else {
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

/// Render goals with adaptive grid layout.
/// Rows = goals (subproofs), Columns = temporal states (Previous/Current/Next).
fn render_goals(frame: &mut Frame, app: &App, area: Rect) {
    if app.goals().is_empty() {
        frame.render_widget(
            Paragraph::new("No goals").style(Style::new().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let layout = determine_column_layout(app, area.width);

    if layout == ColumnLayout::Single {
        render_goals_single_column(frame, app, area);
        return;
    }

    let rows = Layout::vertical(compute_row_constraints(app.goals())).split(area);

    render_grid_header(frame, rows[0], layout);

    let selection = app.current_selection();
    app.goals().iter().enumerate().for_each(|(goal_idx, _)| {
        render_goal_row(
            frame,
            app,
            rows[goal_idx + 1],
            goal_idx,
            selection.as_ref(),
            layout,
        );
    });
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

/// Render the grid header row with column titles.
fn render_grid_header(frame: &mut Frame, area: Rect, layout: ColumnLayout) {
    let cols = split_row_columns(area, layout);
    if let Some(prev) = cols.previous {
        frame.render_widget(header_label("Previous", false), prev);
    }
    frame.render_widget(
        header_label("Current", layout != ColumnLayout::Single),
        cols.current,
    );
    if let Some(next) = cols.next {
        frame.render_widget(header_label("Next", false), next);
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
    app: &App,
    area: Rect,
    goal_idx: usize,
    selection: Option<&SelectableItem>,
    layout: ColumnLayout,
) {
    let cols = split_row_columns(area, layout);
    if let Some(prev) = cols.previous {
        render_goal_cell(frame, app, prev, goal_idx, CellKind::Previous, None);
    }
    render_goal_cell(
        frame,
        app,
        cols.current,
        goal_idx,
        CellKind::Current,
        selection,
    );
    if let Some(next) = cols.next {
        render_goal_cell(frame, app, next, goal_idx, CellKind::Next, None);
    }
}

/// Render a goal cell with optional selection highlighting.
/// For Current cells, pass selection to enable highlighting; for Previous/Next,
/// pass None.
fn render_goal_cell(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    goal_idx: usize,
    kind: CellKind,
    selection: Option<&SelectableItem>,
) {
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

    // All columns use CellKind::Current to show their own diff markers
    // (the goals from Lean already have diff info relative to their position)
    let lines = build_goal_lines(goal, goal_idx, selection, CellKind::Current, app.filters);
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

fn build_goal_lines(
    goal: &Goal,
    goal_idx: usize,
    selection: Option<&SelectableItem>,
    kind: CellKind,
    filters: HypothesisFilters,
) -> Vec<Line<'static>> {
    let header = Line::from(goal_header(goal, goal_idx))
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let hyp_lines =
        hypothesis_indices(goal.hyps.len(), filters.reverse_order).filter_map(|hyp_idx| {
            let hyp = &goal.hyps[hyp_idx];
            (!should_skip_hyp(hyp, kind) && filters.should_show(hyp)).then(|| {
                let is_selected =
                    selection == Some(&SelectableItem::Hypothesis { goal_idx, hyp_idx });
                render_hypothesis_line(hyp, is_selected, kind, filters)
            })
        });

    let is_target_selected = selection == Some(&SelectableItem::GoalTarget { goal_idx });
    let target = render_target_line(goal, is_target_selected, kind);

    std::iter::once(header)
        .chain(hyp_lines)
        .chain(std::iter::once(target))
        .collect()
}

/// Render single column with inline diff markers (narrow terminal).
fn render_goals_single_column(frame: &mut Frame, app: &App, area: Rect) {
    let selection = app.current_selection();

    let error_lines = app.error.iter().flat_map(|error| {
        [
            Line::from(format!("Error: {error}")).fg(Color::Red),
            Line::default(),
        ]
    });

    let goal_lines = app.goals().iter().enumerate().flat_map(|(goal_idx, goal)| {
        let header = Line::from(goal_header(goal, goal_idx))
            .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        let hyp_lines = hypothesis_indices(goal.hyps.len(), app.filters.reverse_order).filter_map(
            move |hyp_idx| {
                let hyp = &goal.hyps[hyp_idx];
                app.filters.should_show(hyp).then(|| {
                    let is_selected =
                        selection == Some(SelectableItem::Hypothesis { goal_idx, hyp_idx });
                    render_hypothesis_line(hyp, is_selected, CellKind::Current, app.filters)
                })
            },
        );

        let is_target_selected = selection == Some(SelectableItem::GoalTarget { goal_idx });
        let target = render_target_line(goal, is_target_selected, CellKind::Current);

        std::iter::once(header)
            .chain(hyp_lines)
            .chain([target, Line::default()])
    });

    let lines: Vec<Line> = error_lines.chain(goal_lines).collect();
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

fn render_status_bar(frame: &mut Frame, area: Rect, filters: HypothesisFilters) {
    const KEYBINDINGS: &[(&str, &str)] = &[
        ("?", "help"),
        ("j/k", "nav"),
        ("Enter", "go"),
        ("q", "quit"),
    ];

    let separator = Span::raw(" │ ");
    let keybind_spans = KEYBINDINGS.iter().enumerate().flat_map(|(i, (key, desc))| {
        let prefix = (i > 0).then(|| separator.clone());
        prefix.into_iter().chain([
            Span::styled(*key, Style::new().fg(Color::Cyan)),
            Span::raw(format!(": {desc}")),
        ])
    });

    let filter_status = build_filter_status(filters);
    let filter_span = (!filter_status.is_empty())
        .then(|| Span::styled(format!(" [{filter_status}]"), Style::new().fg(Color::Green)));

    let spans: Vec<Span> = keybind_spans.chain(filter_span).collect();
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn build_filter_status(filters: HypothesisFilters) -> String {
    [
        (filters.hide_instances, 'i'),
        (filters.hide_types, 't'),
        (filters.hide_inaccessible, 'a'),
        (filters.hide_let_values, 'l'),
        (filters.reverse_order, 'r'),
    ]
    .into_iter()
    .filter_map(|(enabled, c)| enabled.then_some(c))
    .collect()
}

/// Render the help popup overlay in bottom-right corner.
fn render_help_popup(frame: &mut Frame) {
    const KEYBINDINGS: &[(&str, &str)] = &[
        ("j/k", "navigate"),
        ("Enter", "go to definition"),
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

    let frame_area = frame.area();
    let width = 28u16;
    #[allow(clippy::cast_possible_truncation)]
    let height = (KEYBINDINGS.len() as u16) + 2;
    let x = frame_area.width.saturating_sub(width + 1);
    let y = frame_area.height.saturating_sub(height + 2);
    let area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, area);

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

    frame.render_widget(Paragraph::new(help_lines).block(block), area);
}
