//! Goals column widget for temporal comparison views (Before/After mode).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};

use super::{
    goal_box::{GoalBox, GoalBoxState},
    tree_colors, ClickRegion, HypothesisFilters, Selection,
};
use crate::{
    lean_rpc::Goal,
    tui::widgets::{layout_metrics::LayoutMetrics, theme::Theme},
};

/// State for the goals column widget (render artifacts only).
#[derive(Default)]
pub struct GoalsColumnState {
    click_regions: Vec<ClickRegion>,
    goal_box_states: Vec<GoalBoxState>,
}

impl GoalsColumnState {
    /// Get the click regions computed during the last render.
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

/// Widget for rendering a column of goals with bordered goal boxes.
pub struct GoalsColumn<'a> {
    title: &'a str,
    goals: &'a [Goal],
    filters: HypothesisFilters,
    selection: Option<Selection>,
    is_current: bool,
    node_id: Option<u32>,
    active_goal_name: Option<&'a str>,
}

impl<'a> GoalsColumn<'a> {
    pub const fn new(
        title: &'a str,
        goals: &'a [Goal],
        filters: HypothesisFilters,
        selection: Option<Selection>,
        is_current: bool,
        node_id: Option<u32>,
        active_goal_name: Option<&'a str>,
    ) -> Self {
        Self {
            title,
            goals,
            filters,
            selection,
            is_current,
            node_id,
            active_goal_name,
        }
    }

}

fn goal_border_color(goal: &Goal, active_goal_name: Option<&str>) -> Option<Color> {
    let active = active_goal_name?;
    let is_active = goal.user_name.as_deref() == Some(active);
    Some(if is_active {
        tree_colors::CURRENT_BORDER
    } else {
        tree_colors::INCOMPLETE_BORDER
    })
}

impl StatefulWidget for GoalsColumn<'_> {
    type State = GoalsColumnState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        let block = create_border_block(self.title, self.is_current);
        let inner = block.inner(area);
        block.render(area, buf);

        if self.goals.is_empty() {
            render_empty_message(inner, buf, self.is_current);
            return;
        }

        // Each goal box gets exactly the height needed for its content
        let constraints: Vec<Constraint> = self
            .goals
            .iter()
            .map(|goal| {
                let visible_hyps = goal.hyps.iter().filter(|h| self.filters.should_show(h)).count();
                Constraint::Length(LayoutMetrics::goal_box_height(visible_hyps))
            })
            .collect();

        let areas = Layout::vertical(constraints).split(inner);

        // Ensure we have enough goal box states
        state
            .goal_box_states
            .resize_with(self.goals.len(), GoalBoxState::default);

        // Render each goal as a GoalBox
        let selection = if self.is_current {
            self.selection
        } else {
            None
        };
        let node_id = if self.is_current { self.node_id } else { None };

        for (idx, (goal, goal_area)) in self.goals.iter().zip(areas.iter()).enumerate() {
            let border_color = goal_border_color(goal, self.active_goal_name);
            let goal_box = GoalBox::new(
                goal,
                idx,
                selection,
                self.filters,
                node_id,
                border_color,
            );

            goal_box.render(*goal_area, buf, &mut state.goal_box_states[idx]);

            if self.is_current {
                state
                    .click_regions
                    .extend(state.goal_box_states[idx].click_regions().iter().cloned());
            }
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
