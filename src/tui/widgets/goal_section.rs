//! Goals section - the bottom portion of the tactic tree view.

use std::collections::HashSet;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Table, Widget,
    },
};

use crate::{
    lean_rpc::{GoalInfo, ProofState},
    tui::widgets::{
        diff_text::TaggedTextExt, layout_metrics::LayoutMetrics, theme::Theme, ClickRegion,
        Selection,
    },
};

/// State for the goal section widget.
#[derive(Default)]
pub struct GoalSectionState {
    goals: Vec<GoalInfo>,
    selection: Option<Selection>,
    spawned_goal_ids: HashSet<String>,
    /// Node ID for creating click region selections.
    node_id: Option<u32>,
    /// Name of the goal the cursor's tactic is working on.
    active_goal_name: Option<String>,
    click_regions: Vec<ClickRegion>,
    scroll_state: ScrollbarState,
    vertical_scroll: usize,
    /// Whether this pane is currently focused.
    is_focused: bool,
}

/// Lay out information for tracking click regions in the goal section.
struct GoalClickLayout {
    area: Rect,
    goal_count: usize,
}

impl GoalSectionState {
    /// Update the state with new data.
    pub fn update(
        &mut self,
        state: &ProofState,
        selection: Option<Selection>,
        spawned_goal_ids: HashSet<String>,
        node_id: Option<u32>,
        active_goal_name: Option<String>,
    ) {
        self.goals.clone_from(&state.goals);
        self.selection = selection;
        self.spawned_goal_ids = spawned_goal_ids;
        self.node_id = node_id;
        self.active_goal_name = active_goal_name;
    }

    /// Get the click regions computed during the last render.
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }

    /// Set focus state.
    pub const fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;
    }
}

/// Widget for rendering the goal section.
pub struct GoalSection;

impl StatefulWidget for GoalSection {
    type State = GoalSectionState;

    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        let border_style = if state.is_focused {
            Style::new().fg(Theme::BORDER_FOCUSED)
        } else {
            Theme::DIM
        };
        let title = if state.is_focused {
            "▶ Goals "
        } else {
            " Goals "
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title)
            .title_style(Style::new().fg(Color::Red));

        let inner = block.inner(area);
        block.render(area, buf);

        if state.goals.is_empty() {
            Paragraph::new("✓ All goals completed!")
                .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD))
                .render(inner, buf);
            return;
        }

        // Calculate scroll position from selection
        if let Some(Selection::Goal { goal_idx, .. }) = state.selection {
            state.vertical_scroll = LayoutMetrics::scroll_position(goal_idx);
        }

        let total_goals = state.goals.len();
        let viewport_height = inner.height as usize;

        // Update scrollbar state
        state.scroll_state = state
            .scroll_state
            .content_length(total_goals)
            .position(state.vertical_scroll);

        // Track click regions for goals
        if let Some(node_id) = state.node_id {
            let layout = GoalClickLayout {
                area: inner,
                goal_count: state.goals.len(),
            };
            track_goal_click_regions(&mut state.click_regions, node_id, &layout);
        }

        let rows: Vec<Row<'static>> = state
            .goals
            .iter()
            .enumerate()
            .map(|(goal_idx, goal)| {
                let is_selected = matches!(
                    state.selection,
                    Some(Selection::Goal { goal_idx: gi, .. }) if gi == goal_idx
                );
                let is_spawned = goal
                    .username
                    .as_str()
                    .is_some_and(|name| state.spawned_goal_ids.contains(name));
                let is_active = goal
                    .username
                    .as_str()
                    .is_some_and(|name| state.active_goal_name.as_deref() == Some(name));
                goal_row(goal, is_selected, is_spawned, is_active)
            })
            .collect();

        Widget::render(
            Table::new(
                rows,
                [
                    Constraint::Length(12), // case label
                    Constraint::Fill(1),    // goal type
                ],
            )
            .column_spacing(1),
            inner,
            buf,
        );

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

fn track_goal_click_regions(
    click_regions: &mut Vec<ClickRegion>,
    node_id: u32,
    layout: &GoalClickLayout,
) {
    for goal_idx in 0..layout.goal_count {
        let y = layout.area.y + goal_idx as u16;
        if y >= layout.area.y + layout.area.height {
            break;
        }
        click_regions.push(ClickRegion {
            area: Rect::new(layout.area.x, y, layout.area.width, 1),
            selection: Selection::Goal { node_id, goal_idx },
        });
    }
}

fn goal_row(
    goal: &GoalInfo,
    is_selected: bool,
    _is_spawned: bool,
    is_active: bool,
) -> Row<'static> {
    let base_color = if is_active {
        Theme::CURRENT_NODE_BORDER
    } else {
        Theme::INCOMPLETE_NODE_BORDER
    };
    let style = if is_selected {
        Style::new().bg(Theme::SELECTION_BG).fg(base_color)
    } else {
        Style::new().fg(base_color)
    };

    // Column 1: case label (e.g. "Expected:" or "h.mpr:")
    let case_label = goal
        .username
        .as_str()
        .map_or(String::new(), |n| format!("{n}: "));
    let col1 = Cell::from(Line::from(vec![Span::styled(case_label, style)]));

    // Column 2: goal type (with diff highlighting)
    let mut spans = vec![Span::styled("⊢ ", style)];
    spans.extend(goal.type_.to_spans(style));
    let col2 = Cell::from(Text::from(Line::from(spans)));

    Row::new(vec![col1, col2])
}
