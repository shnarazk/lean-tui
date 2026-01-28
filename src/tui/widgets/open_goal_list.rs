//! List of open goals widget.

use std::iter;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Color,
    widgets::{Paragraph, StatefulWidget, Widget},
    Frame,
};

use super::{
    goal_box::{GoalBox, GoalBoxState},
    tree_colors, ClickRegion, HypothesisFilters, Selection,
};
use crate::{
    lean_rpc::Goal,
    tui::widgets::{layout_metrics::LayoutMetrics, theme::Theme},
};

/// Widget for rendering a list of open goals.
pub struct OpenGoalList<'a> {
    goals: &'a [Goal],
    selection: Option<Selection>,
    filters: HypothesisFilters,
    /// Node ID for creating click region selections.
    node_id: Option<u32>,
    /// Name of the goal the cursor's tactic is working on.
    active_goal_name: Option<&'a str>,
}

/// Mutable state for `OpenGoalList` that tracks click regions.
#[derive(Default)]
pub struct OpenGoalListState {
    click_regions: Vec<ClickRegion>,
    goal_box_states: Vec<GoalBoxState>,
}

impl OpenGoalListState {
    #[allow(dead_code)]
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

impl<'a> OpenGoalList<'a> {
    pub const fn new(
        goals: &'a [Goal],
        selection: Option<Selection>,
        filters: HypothesisFilters,
        node_id: Option<u32>,
        active_goal_name: Option<&'a str>,
    ) -> Self {
        Self {
            goals,
            selection,
            filters,
            node_id,
            active_goal_name,
        }
    }

    /// Render using Frame (convenience method for non-stateful usage).
    pub fn render_to_frame(&self, frame: &mut Frame, area: Rect) -> Vec<ClickRegion> {
        let mut state = OpenGoalListState::default();
        frame.render_stateful_widget(
            OpenGoalList::new(
                self.goals,
                self.selection,
                self.filters,
                self.node_id,
                self.active_goal_name,
            ),
            area,
            &mut state,
        );
        state.click_regions
    }

    fn min_goal_height(&self, goal: &Goal) -> u16 {
        let visible_hyps = goal
            .hyps
            .iter()
            .filter(|h| self.filters.should_show(h))
            .count();
        LayoutMetrics::goal_box_height(visible_hyps)
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

impl StatefulWidget for OpenGoalList<'_> {
    type State = OpenGoalListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        if self.goals.is_empty() {
            Paragraph::new("No goals")
                .style(Theme::DIM)
                .render(area, buf);
            return;
        }

        // Build constraints for each goal
        let constraints: Vec<Constraint> = self
            .goals
            .iter()
            .map(|goal| Constraint::Min(self.min_goal_height(goal)))
            .chain(iter::once(Constraint::Fill(1)))
            .collect();

        let areas = Layout::vertical(constraints).split(area);

        // Ensure we have enough goal box states
        state
            .goal_box_states
            .resize_with(self.goals.len(), GoalBoxState::default);

        // Render each goal directly
        for (idx, (goal, goal_area)) in self.goals.iter().zip(areas.iter()).enumerate() {
            let border_color = goal_border_color(goal, self.active_goal_name);
            let goal_box = GoalBox::new(
                goal,
                idx,
                self.selection,
                self.filters,
                self.node_id,
                border_color,
            );

            goal_box.render(*goal_area, buf, &mut state.goal_box_states[idx]);

            // Collect click regions from this goal box
            state
                .click_regions
                .extend(state.goal_box_states[idx].click_regions().iter().cloned());
        }
    }
}
