//! Goals column widget for temporal comparison views (Before/After mode).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{
        Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget,
    },
};

use super::{
    diff_text::{render_hypothesis_line, render_target_line},
    ClickRegion, HypothesisFilters, Selection,
};
use crate::{lean_rpc::Goal, tui::widgets::theme::Theme};

/// State for the goals column widget (render artifacts only).
#[derive(Default)]
pub struct GoalsColumnState {
    click_regions: Vec<ClickRegion>,
    scroll_state: ScrollbarState,
    vertical_scroll: usize,
}

impl GoalsColumnState {
    /// Get the click regions computed during the last render.
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

fn push_click_region_if_visible(
    click_regions: &mut Vec<ClickRegion>,
    inner: Rect,
    viewport_height: usize,
    line_idx: usize,
    make_selection: impl FnOnce() -> Selection,
) {
    if line_idx >= viewport_height {
        return;
    }
    click_regions.push(ClickRegion {
        area: Rect::new(inner.x, inner.y + line_idx as u16, inner.width, 1),
        selection: make_selection(),
    });
}

/// Widget for rendering a column of goals with diff markers.
pub struct GoalsColumn<'a> {
    title: &'a str,
    goals: &'a [Goal],
    filters: HypothesisFilters,
    selection: Option<Selection>,
    is_current: bool,
    node_id: Option<u32>,
}

impl<'a> GoalsColumn<'a> {
    pub const fn new(
        title: &'a str,
        goals: &'a [Goal],
        filters: HypothesisFilters,
        selection: Option<Selection>,
        is_current: bool,
        node_id: Option<u32>,
    ) -> Self {
        Self {
            title,
            goals,
            filters,
            selection,
            is_current,
            node_id,
        }
    }

    fn track_click_regions(
        &self,
        click_regions: &mut Vec<ClickRegion>,
        inner: Rect,
        viewport_height: usize,
    ) {
        let Some(node_id) = self.node_id else {
            return;
        };

        let mut line_idx = 0;

        for (goal_idx, goal) in self.goals.iter().enumerate() {
            line_idx = self.track_goal_click_regions(
                click_regions,
                inner,
                viewport_height,
                node_id,
                goal_idx,
                goal,
                line_idx,
            );

            // Separator line
            if goal_idx < self.goals.len() - 1 {
                line_idx += 1;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn track_goal_click_regions(
        &self,
        click_regions: &mut Vec<ClickRegion>,
        inner: Rect,
        viewport_height: usize,
        node_id: u32,
        goal_idx: usize,
        goal: &Goal,
        mut line_idx: usize,
    ) -> usize {
        for (hyp_idx, hyp) in goal.hyps.iter().enumerate() {
            if !self.filters.should_show(hyp) {
                continue;
            }
            push_click_region_if_visible(click_regions, inner, viewport_height, line_idx, || {
                Selection::Hyp { node_id, hyp_idx }
            });
            line_idx += 1;
        }

        // Target line
        push_click_region_if_visible(click_regions, inner, viewport_height, line_idx, || {
            Selection::Goal { node_id, goal_idx }
        });
        line_idx + 1
    }
}

impl StatefulWidget for GoalsColumn<'_> {
    type State = GoalsColumnState;

    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        let block = create_border_block(self.title, self.is_current);
        let inner = block.inner(area);
        block.render(area, buf);

        if self.goals.is_empty() {
            render_empty_message(inner, buf, self.is_current);
            return;
        }

        // Calculate scroll position from selection
        if let Some(selection) = self.selection {
            state.vertical_scroll = calculate_line_position(self.goals, self.filters, selection);
        }

        let lines = build_goal_lines(self.goals, self.filters, self.selection, self.is_current);
        let total_lines = lines.len();
        let viewport_height = inner.height as usize;

        // Update scrollbar state
        state.scroll_state = state
            .scroll_state
            .content_length(total_lines)
            .position(state.vertical_scroll);

        // Track click regions for current column
        if self.is_current {
            self.track_click_regions(&mut state.click_regions, inner, viewport_height);
        }

        Paragraph::new(lines).render(inner, buf);

        // Render scrollbar if content overflows
        if total_lines > viewport_height && inner.width > 1 {
            render_scrollbar(area, buf, &mut state.scroll_state);
        }
    }
}

fn create_border_block(title: &str, is_current: bool) -> Block<'static> {
    let border_color = if is_current {
        Theme::TITLE_GOAL
    } else {
        Theme::BORDER
    };
    let title_style = if is_current {
        Style::new()
            .fg(Theme::TITLE_GOAL)
            .add_modifier(Modifier::BOLD)
    } else {
        Theme::DIM
    };

    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(border_color))
        .title(format!(" {title} "))
        .title_style(title_style)
}

fn render_empty_message(area: Rect, buf: &mut Buffer, is_current: bool) {
    let msg = if is_current { "No goals" } else { "No data" };
    Paragraph::new(msg).style(Theme::DIM).render(area, buf);
}

fn build_goal_lines(
    goals: &[Goal],
    filters: HypothesisFilters,
    selection: Option<Selection>,
    is_current: bool,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for (goal_idx, goal) in goals.iter().enumerate() {
        // Render hypotheses
        for (hyp_idx, hyp) in goal.hyps.iter().enumerate() {
            if !filters.should_show(hyp) {
                continue;
            }

            let is_selected = is_current && is_hyp_selected(selection, hyp_idx);
            lines.push(render_hypothesis_line(hyp, is_selected, filters));
        }

        // Render goal target
        let is_selected = is_current && is_goal_selected(selection, goal_idx);
        lines.push(render_target_line(goal, is_selected));

        // Separator between goals
        if goal_idx < goals.len() - 1 {
            lines.push(Line::from(""));
        }
    }

    lines
}

#[allow(clippy::missing_const_for_fn)] // matches! with guard cannot be const
fn is_hyp_selected(selection: Option<Selection>, hyp_idx: usize) -> bool {
    matches!(selection, Some(Selection::Hyp { hyp_idx: hi, .. }) if hi == hyp_idx)
}

#[allow(clippy::missing_const_for_fn)] // matches! with guard cannot be const
fn is_goal_selected(selection: Option<Selection>, goal_idx: usize) -> bool {
    matches!(selection, Some(Selection::Goal { goal_idx: gi, .. }) if gi == goal_idx)
}

fn calculate_line_position(
    goals: &[Goal],
    filters: HypothesisFilters,
    selection: Selection,
) -> usize {
    let mut line_count = 0;

    match selection {
        Selection::Hyp { hyp_idx, .. } => {
            // For this column view, we find the hypothesis by index in the first goal
            if let Some(goal) = goals.first() {
                return line_count + find_hypothesis_line(goal, filters, hyp_idx);
            }
        }
        Selection::Goal { goal_idx, .. } => {
            for (g_idx, goal) in goals.iter().enumerate() {
                if g_idx == goal_idx {
                    line_count += goal.hyps.iter().filter(|h| filters.should_show(h)).count();
                    return line_count;
                }
                line_count += count_goal_lines(goal, filters);
            }
        }
        // Tree-specific selections aren't used in this column view
        Selection::InitialHyp { .. } | Selection::Theorem => {}
    }

    line_count
}

fn find_hypothesis_line(goal: &Goal, filters: HypothesisFilters, target_hyp_idx: usize) -> usize {
    goal.hyps
        .iter()
        .enumerate()
        .filter(|(_, hyp)| filters.should_show(hyp))
        .position(|(h_idx, _)| h_idx == target_hyp_idx)
        .unwrap_or(0)
}

fn count_goal_lines(goal: &Goal, filters: HypothesisFilters) -> usize {
    let hyp_count = goal.hyps.iter().filter(|h| filters.should_show(h)).count();
    hyp_count + 1 + 1 // hypotheses + target + separator
}

fn render_scrollbar(area: Rect, buf: &mut Buffer, scroll_state: &mut ScrollbarState) {
    let scrollbar_area = Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y + 1, // Skip top border
        width: 1,
        height: area.height.saturating_sub(2), // Skip top and bottom borders
    };
    Scrollbar::new(ScrollbarOrientation::VerticalRight).render(scrollbar_area, buf, scroll_state);
}
