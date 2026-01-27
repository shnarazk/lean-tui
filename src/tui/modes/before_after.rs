//! Before/After mode - three-column temporal comparison view.

use std::iter;

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::{Backend, Mode};
use crate::{
    lean_rpc::Goal,
    tui::components::{
        diff_style, hypothesis_indices, render_error, Component, DiffState, FilterToggle,
        HypothesisFilters, KeyMouseEvent, SelectableItem, SelectionState, TaggedTextExt,
    },
    tui_ipc::DefinitionInfo,
};

/// Input for updating the Before/After mode.
pub struct BeforeAfterModeInput {
    pub previous_goals: Option<Vec<Goal>>,
    pub current_goals: Vec<Goal>,
    pub next_goals: Option<Vec<Goal>>,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
}

/// Before/After display mode - temporal comparison of goal states.
pub struct BeforeAfterMode {
    previous_goals: Option<Vec<Goal>>,
    current_goals: Vec<Goal>,
    next_goals: Option<Vec<Goal>>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    filters: HypothesisFilters,
    selection: SelectionState,
    show_previous: bool,
    show_next: bool,
}

impl Default for BeforeAfterMode {
    fn default() -> Self {
        Self {
            previous_goals: None,
            current_goals: Vec::new(),
            next_goals: None,
            definition: None,
            error: None,
            filters: HypothesisFilters::default(),
            selection: SelectionState::default(),
            show_previous: true, // Show previous column by default
            show_next: false,
        }
    }
}

impl BeforeAfterMode {
    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

    /// Whether the previous column should be shown.
    pub const fn show_previous(&self) -> bool {
        self.show_previous
    }

    /// Whether the next column should be shown.
    pub const fn show_next(&self) -> bool {
        self.show_next
    }

    fn selectable_items(&self) -> Vec<SelectableItem> {
        self.current_goals
            .iter()
            .enumerate()
            .flat_map(|(goal_idx, goal)| {
                let hyp_items = hypothesis_indices(goal.hyps.len(), self.filters.reverse_order)
                    .filter(|&hyp_idx| self.filters.should_show(&goal.hyps[hyp_idx]))
                    .map(move |hyp_idx| SelectableItem::Hypothesis { goal_idx, hyp_idx });

                hyp_items.chain(iter::once(SelectableItem::GoalTarget { goal_idx }))
            })
            .collect()
    }
}

impl Component for BeforeAfterMode {
    type Input = BeforeAfterModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        let goals_changed = self.current_goals != input.current_goals;
        self.previous_goals = input.previous_goals;
        self.current_goals = input.current_goals;
        self.next_goals = input.next_goals;
        self.definition = input.definition;
        self.error = input.error;
        if goals_changed {
            self.selection.reset(self.selectable_items().len());
        }
    }

    fn handle_event(&mut self, event: Self::Event) -> bool {
        let items = self.selectable_items();
        match event {
            KeyMouseEvent::Key(key) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.selection.select_next(items.len());
                    true
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.selection.select_previous(items.len());
                    true
                }
                KeyCode::Char('i') => {
                    self.filters.toggle(FilterToggle::Instances);
                    true
                }
                KeyCode::Char('a') => {
                    self.filters.toggle(FilterToggle::Inaccessible);
                    true
                }
                KeyCode::Char('l') => {
                    self.filters.toggle(FilterToggle::LetValues);
                    true
                }
                KeyCode::Char('r') => {
                    self.filters.toggle(FilterToggle::ReverseOrder);
                    true
                }
                KeyCode::Char('p') => {
                    self.show_previous = !self.show_previous;
                    true
                }
                KeyCode::Char('n') => {
                    self.show_next = !self.show_next;
                    true
                }
                _ => false,
            },
            KeyMouseEvent::Mouse(mouse) => {
                mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && self.selection.handle_click(mouse.column, mouse.row, &items)
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.selection.clear_regions();

        let content_area = render_error(frame, area, self.error.as_deref());

        // Three-column layout
        let has_prev = self.previous_goals.is_some();
        let has_next = self.next_goals.is_some();

        let constraints = match (has_prev, has_next) {
            (true, true) => vec![
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ],
            (true, false) => vec![Constraint::Percentage(35), Constraint::Percentage(65)],
            (false, true) => vec![Constraint::Percentage(65), Constraint::Percentage(35)],
            (false, false) => vec![Constraint::Percentage(100)],
        };

        let columns = Layout::horizontal(constraints).split(content_area);
        let mut col_idx = 0;

        // Clone goals to avoid borrow issues
        let prev_goals = self.previous_goals.clone();
        let current_goals = self.current_goals.clone();
        let next_goals = self.next_goals.clone();

        // Previous column
        if let Some(ref goals) = prev_goals {
            self.render_column(frame, columns[col_idx], "Previous", goals, false);
            col_idx += 1;
        }

        // Current column (always shown)
        self.render_column(frame, columns[col_idx], "Current", &current_goals, true);
        col_idx += 1;

        // Next column
        if let Some(ref goals) = next_goals {
            self.render_column(frame, columns[col_idx], "Next", goals, false);
        }
    }
}

fn diff_marker(is_inserted: bool, is_removed: bool) -> Span<'static> {
    if is_inserted {
        Span::styled("[+]", Style::new().fg(Color::Green))
    } else if is_removed {
        Span::styled("[-]", Style::new().fg(Color::Red))
    } else {
        Span::styled("   ", Style::new())
    }
}

const fn selection_indicator(is_selected: bool) -> &'static str {
    if is_selected {
        "▶ "
    } else {
        "  "
    }
}

impl BeforeAfterMode {
    #[allow(clippy::too_many_arguments)]
    fn render_column(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        goals: &[Goal],
        is_current: bool,
    ) {
        let border_color = if is_current {
            Color::Cyan
        } else {
            Color::DarkGray
        };
        let title_style = if is_current {
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(border_color))
            .title(format!(" {title} "))
            .title_style(title_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if goals.is_empty() {
            let msg = if is_current { "No goals" } else { "No data" };
            frame.render_widget(
                Paragraph::new(msg).style(Style::new().fg(Color::DarkGray)),
                inner,
            );
            return;
        }

        let selection = self.current_selection();
        let lines = self.build_column_lines(goals, is_current, selection, inner);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn build_column_lines(
        &mut self,
        goals: &[Goal],
        is_current: bool,
        selection: Option<SelectableItem>,
        inner: Rect,
    ) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        for (goal_idx, goal) in goals.iter().enumerate() {
            self.render_goal_hypotheses(&mut lines, goal, goal_idx, is_current, selection, inner);
            self.render_goal_target(&mut lines, goal, goal_idx, is_current, selection, inner);

            if goal_idx < goals.len() - 1 {
                lines.push(Line::from(""));
            }
        }

        lines
    }

    fn render_goal_hypotheses(
        &mut self,
        lines: &mut Vec<Line<'static>>,
        goal: &Goal,
        goal_idx: usize,
        is_current: bool,
        selection: Option<SelectableItem>,
        inner: Rect,
    ) {
        for (hyp_idx, hyp) in goal.hyps.iter().enumerate() {
            if !self.filters.should_show(hyp) {
                continue;
            }

            let is_selected =
                is_current && selection == Some(SelectableItem::Hypothesis { goal_idx, hyp_idx });
            let names = hyp.names.join(", ");

            // Build diff state for fine-grained coloring
            let diff_state = DiffState {
                is_inserted: hyp.is_inserted,
                is_removed: hyp.is_removed,
                has_diff: hyp.type_.has_any_diff(),
            };
            let diff = diff_style(&diff_state, is_selected, Color::White);

            // Build line with spans
            let mut spans = vec![
                diff_marker(hyp.is_inserted, hyp.is_removed),
                Span::styled(
                    selection_indicator(is_selected),
                    Style::new().fg(Color::Cyan),
                ),
                Span::styled(format!("{names}: "), diff.style),
            ];
            spans.extend(hyp.type_.to_spans(diff.style));

            lines.push(Line::from(spans));

            self.track_click_region(
                lines,
                inner,
                is_current,
                SelectableItem::Hypothesis { goal_idx, hyp_idx },
            );
        }
    }

    fn render_goal_target(
        &mut self,
        lines: &mut Vec<Line<'static>>,
        goal: &Goal,
        goal_idx: usize,
        is_current: bool,
        selection: Option<SelectableItem>,
        inner: Rect,
    ) {
        let is_selected = is_current && selection == Some(SelectableItem::GoalTarget { goal_idx });

        // Build diff state for fine-grained coloring
        let diff_state = DiffState {
            is_inserted: goal.is_inserted,
            is_removed: goal.is_removed,
            has_diff: goal.target.has_any_diff(),
        };
        let diff = diff_style(&diff_state, is_selected, Color::Cyan);

        // Build line with spans
        let mut spans = vec![
            diff_marker(goal.is_inserted, goal.is_removed),
            Span::styled(
                selection_indicator(is_selected),
                Style::new().fg(Color::Cyan),
            ),
            Span::styled(goal.prefix.clone(), diff.style),
        ];
        spans.extend(goal.target.to_spans(diff.style));

        lines.push(Line::from(spans));

        self.track_click_region(
            lines,
            inner,
            is_current,
            SelectableItem::GoalTarget { goal_idx },
        );
    }

    fn track_click_region(
        &mut self,
        lines: &[Line<'static>],
        inner: Rect,
        is_current: bool,
        item: SelectableItem,
    ) {
        if is_current && lines.len() <= inner.height as usize {
            #[allow(clippy::cast_possible_truncation)]
            let y = inner.y + (lines.len() - 1) as u16;
            self.selection
                .add_region(Rect::new(inner.x, y, inner.width, 1), item);
        }
    }
}

impl Mode for BeforeAfterMode {
    type Model = BeforeAfterModeInput;

    const NAME: &'static str = "Before/After";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[
        ("p", "prev"),
        ("n", "next"),
        ("i", "inst"),
        ("a", "access"),
        ("l", "let"),
        ("r", "rev"),
    ];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[
        FilterToggle::Instances,
        FilterToggle::Inaccessible,
        FilterToggle::LetValues,
        FilterToggle::ReverseOrder,
    ];
    const BACKENDS: &'static [Backend] = &[Backend::LeanRpc];

    fn current_selection(&self) -> Option<SelectableItem> {
        self.selection
            .current_selection(&self.selectable_items())
            .copied()
    }
}
