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

mod tree_chars {
    pub const MIDDLE: &str = "├── ";
    pub const LAST: &str = "╰── ";
    pub const VERTICAL: &str = "│   ";
    pub const EMPTY: &str = "    ";
}

enum TreeElement {
    Label(Line<'static>),
    Goal { idx: usize, prefix: String },
}

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

    fn build_tree_elements(&self) -> Vec<TreeElement> {
        let mut elements = Vec::new();

        let has_named_goals = self.goals.iter().any(|g| g.user_name.is_some());
        let use_tree = self.goals.len() > 1 && has_named_goals;

        if use_tree {
            self.append_goals_as_tree("", &mut elements);
        } else {
            self.append_goals_flat(&mut elements);
        }

        elements
    }

    fn append_goals_flat(&self, elements: &mut Vec<TreeElement>) {
        elements.extend(
            self.goals
                .iter()
                .enumerate()
                .map(|(idx, _)| TreeElement::Goal {
                    idx,
                    prefix: String::new(),
                }),
        );
    }

    fn append_goals_as_tree(&self, base_prefix: &str, elements: &mut Vec<TreeElement>) {
        let last_idx = self.goals.len().saturating_sub(1);

        for (idx, goal) in self.goals.iter().enumerate() {
            let is_last = idx == last_idx;
            let (connector, continuation) = if is_last {
                (tree_chars::LAST, tree_chars::EMPTY)
            } else {
                (tree_chars::MIDDLE, tree_chars::VERTICAL)
            };

            if let Some(case_name) = &goal.user_name {
                elements.push(TreeElement::Label(render_case_label(
                    base_prefix,
                    connector,
                    case_name,
                )));
            }

            elements.push(TreeElement::Goal {
                idx,
                prefix: format!("{base_prefix}{continuation}"),
            });
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

        let elements = self.build_tree_elements();

        // Build flexible constraints
        let constraints: Vec<Constraint> = elements
            .iter()
            .map(|e| match e {
                TreeElement::Label(_) => Constraint::Length(1),
                TreeElement::Goal { idx, .. } => {
                    Constraint::Min(self.min_goal_height(&self.goals[*idx]))
                }
            })
            .chain(iter::once(Constraint::Fill(1)))
            .collect();

        let areas = Layout::vertical(constraints).split(area);

        // Ensure we have enough goal box states
        let goal_count = elements
            .iter()
            .filter(|e| matches!(e, TreeElement::Goal { .. }))
            .count();
        state
            .goal_box_states
            .resize_with(goal_count, GoalBoxState::default);

        let mut goal_state_idx = 0;

        for (element, elem_area) in elements.iter().zip(areas.iter()) {
            match element {
                TreeElement::Label(line) => {
                    Paragraph::new(line.clone()).render(*elem_area, buf);
                }
                TreeElement::Goal { idx, prefix } => {
                    let content_area = layout_with_prefix(*elem_area, prefix, buf);

                    let goal_box = GoalBox::new(
                        &self.goals[*idx],
                        *idx,
                        self.selection,
                        self.filters,
                        self.node_id,
                    );

                    goal_box.render(
                        content_area,
                        buf,
                        &mut state.goal_box_states[goal_state_idx],
                    );

                    // Collect click regions from this goal box
                    state.click_regions.extend(
                        state.goal_box_states[goal_state_idx]
                            .click_regions()
                            .iter()
                            .cloned(),
                    );

                    goal_state_idx += 1;
                }
            }
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

fn render_vertical_prefix(area: Rect, prefix: &str, buf: &mut Buffer) {
    let line = Line::from(Span::styled(prefix.to_string(), Theme::TREE_CHARS));
    let lines: Vec<Line> = iter::repeat_n(line, area.height as usize).collect();
    Paragraph::new(lines).render(area, buf);
}

fn render_case_label(prefix: &str, connector: &str, name: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(prefix.to_string(), Theme::TREE_CHARS),
        Span::styled(connector.to_string(), Theme::TREE_CHARS),
        Span::styled(format!("{name}:"), Theme::CASE_LABEL),
    ])
}
