//! Hierarchical goal tree with case labels.

use std::iter;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, StatefulWidget, Widget},
    Frame,
};

use super::{
    goal_box::{GoalBox, GoalBoxState},
    ClickRegion, HypothesisFilters, Selection,
};
use crate::{
    lean_rpc::Goal,
    tui::widgets::{layout_metrics::LayoutMetrics, theme::Theme},
};

/// Widget for rendering a hierarchical tree of goals.
pub struct GoalTree<'a> {
    goals: &'a [Goal],
    selection: Option<Selection>,
    filters: HypothesisFilters,
    /// Node ID for creating click region selections.
    node_id: Option<u32>,
}

/// Mutable state for `GoalTree` that tracks click regions.
#[derive(Default)]
pub struct GoalTreeState {
    click_regions: Vec<ClickRegion>,
    goal_box_states: Vec<GoalBoxState>,
}

impl GoalTreeState {
    #[allow(dead_code)]
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

impl<'a> GoalTree<'a> {
    pub const fn new(
        goals: &'a [Goal],
        selection: Option<Selection>,
        filters: HypothesisFilters,
        node_id: Option<u32>,
    ) -> Self {
        Self {
            goals,
            selection,
            filters,
            node_id,
        }
    }

    /// Render using Frame (convenience method for non-stateful usage).
    pub fn render_to_frame(&self, frame: &mut Frame, area: Rect) -> Vec<ClickRegion> {
        let mut state = GoalTreeState::default();
        frame.render_stateful_widget(
            GoalTree::new(self.goals, self.selection, self.filters, self.node_id),
            area,
            &mut state,
        );
        state.click_regions
    }

    /// Determine the tree prefix for a goal at the given index.
    /// Returns a prefix string for visual tree structure when multiple goals
    /// exist.
    fn goal_prefix(&self, idx: usize) -> String {
        if self.goals.len() <= 1 {
            return String::new();
        }
        let is_last = idx == self.goals.len() - 1;
        if is_last {
            tree_chars::EMPTY.to_string()
        } else {
            tree_chars::VERTICAL.to_string()
        }
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

impl StatefulWidget for GoalTree<'_> {
    type State = GoalTreeState;

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
            let prefix = self.goal_prefix(idx);
            let content_area = layout_with_prefix(*goal_area, &prefix, buf);

            let goal_box = GoalBox::new(goal, idx, self.selection, self.filters, self.node_id, None);

            goal_box.render(content_area, buf, &mut state.goal_box_states[idx]);

            // Collect click regions from this goal box
            state
                .click_regions
                .extend(state.goal_box_states[idx].click_regions().iter().cloned());
        }
    }
}

fn layout_with_prefix(area: Rect, prefix: &str, buf: &mut Buffer) -> Rect {
    #[allow(clippy::cast_possible_truncation)]
    let prefix_width = prefix.chars().count() as u16;
    let min_content_width = 10;

    if prefix_width > 0 && area.width > prefix_width + min_content_width {
        let [prefix_area, content_area] =
            Layout::horizontal([Constraint::Length(prefix_width), Constraint::Fill(1)]).areas(area);

        render_vertical_prefix(prefix_area, prefix, buf);
        content_area
    } else {
        area
    }
}
