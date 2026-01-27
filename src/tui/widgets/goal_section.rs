//! Goals section - the bottom portion of the Paperproof view.

use std::collections::HashSet;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget,
    },
};

use crate::{
    lean_rpc::Goal,
    tui::widgets::{
        diff_text::{diff_style, DiffState, TaggedTextExt},
        layout_metrics::LayoutMetrics,
        theme::Theme,
        ClickRegion, SelectableItem,
    },
};

/// State for the goal section widget.
#[derive(Default)]
pub struct GoalSectionState {
    goals: Vec<Goal>,
    selection: Option<SelectableItem>,
    spawned_goal_ids: HashSet<String>,
    click_regions: Vec<ClickRegion>,
    scroll_state: ScrollbarState,
    vertical_scroll: usize,
}

impl GoalSectionState {
    /// Update the state with new data.
    pub fn update(
        &mut self,
        goals: Vec<Goal>,
        selection: Option<SelectableItem>,
        spawned_goal_ids: HashSet<String>,
    ) {
        self.goals = goals;
        self.selection = selection;
        self.spawned_goal_ids = spawned_goal_ids;
    }

    /// Get the click regions computed during the last render.
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

/// Widget for rendering the goal section.
pub struct GoalSection;

impl StatefulWidget for GoalSection {
    type State = GoalSectionState;

    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        let block = Block::default()
            .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
            .border_style(Theme::DIM)
            .title(" Goals ")
            .title_style(Style::new().fg(Theme::TITLE_GOAL));

        let inner = block.inner(area);
        block.render(area, buf);

        if state.goals.is_empty() {
            Paragraph::new("✓ All goals completed!")
                .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD))
                .render(inner, buf);
            return;
        }

        // Calculate scroll position from selection
        if let Some(SelectableItem::GoalTarget { goal_idx }) = state.selection {
            state.vertical_scroll = LayoutMetrics::scroll_position(goal_idx);
        }

        let total_goals = state.goals.len();
        let viewport_height = inner.height as usize;

        // Update scrollbar state
        state.scroll_state = state
            .scroll_state
            .content_length(total_goals)
            .position(state.vertical_scroll);

        let lines: Vec<Line<'static>> = state
            .goals
            .iter()
            .enumerate()
            .map(|(goal_idx, goal)| {
                let is_selected = state.selection == Some(SelectableItem::GoalTarget { goal_idx });
                let is_spawned = goal
                    .user_name
                    .as_ref()
                    .is_some_and(|name| state.spawned_goal_ids.contains(name));

                let y = inner.y + goal_idx as u16;
                if y < inner.y + inner.height {
                    state.click_regions.push(ClickRegion {
                        area: Rect::new(inner.x, y, inner.width, 1),
                        item: SelectableItem::GoalTarget { goal_idx },
                    });
                }

                render_goal_line(goal, is_selected, is_spawned, goal_idx, state.goals.len())
            })
            .collect();

        Paragraph::new(lines).render(inner, buf);

        // Render scrollbar if content overflows
        if total_goals > viewport_height && inner.width > 1 {
            let scrollbar_area = Rect {
                x: area.x + area.width.saturating_sub(1),
                y: area.y, // Goals section has bottom border
                width: 1,
                height: area.height.saturating_sub(1),
            };
            Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
                scrollbar_area,
                buf,
                &mut state.scroll_state,
            );
        }
    }
}

fn render_goal_line(
    goal: &Goal,
    is_selected: bool,
    is_spawned: bool,
    idx: usize,
    total: usize,
) -> Line<'static> {
    let state = DiffState {
        is_inserted: goal.is_inserted,
        is_removed: goal.is_removed,
        has_diff: goal.target.has_any_diff(),
    };
    let diff = diff_style(&state, is_selected, Color::Cyan);

    let marker = match (
        is_spawned,
        goal.is_inserted,
        goal.is_removed,
        goal.target.has_any_diff(),
    ) {
        (true, _, _, _) => Span::styled("[⇢]", Style::new().fg(Theme::TITLE_GOAL)),
        (_, true, _, _) => Span::styled("[+]", Theme::INSERTED),
        (_, _, true, _) => Span::styled("[-]", Theme::REMOVED),
        (_, _, _, true) => Span::styled("[~]", Theme::MODIFIED),
        _ => Span::styled("   ", Theme::DIM),
    };

    let selection = if is_selected { "▶ " } else { "  " };
    let goal_num = if total > 1 {
        format!("[{}/{}] ", idx + 1, total)
    } else {
        String::new()
    };
    let case_label = goal
        .user_name
        .as_ref()
        .map_or(String::new(), |n| format!("{n}: "));

    let mut spans = vec![
        marker,
        Span::styled(selection.to_string(), diff.style),
        Span::styled(goal_num, Theme::GOAL_NUMBER),
        Span::styled(case_label, Theme::CASE_LABEL),
        Span::styled(goal.prefix.clone(), diff.style),
    ];
    spans.extend(goal.target.to_spans(diff.style));
    Line::from(spans)
}
