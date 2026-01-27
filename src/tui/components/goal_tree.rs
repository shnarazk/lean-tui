//! Hierarchical goal tree with case labels.

use std::iter;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, StatefulWidget, Widget},
    Frame,
};

use super::{
    goal_box::{GoalBox, GoalBoxState},
    ClickRegion, HypothesisFilters, SelectableItem,
};
use crate::{lean_rpc::Goal, tui_ipc::CaseSplitInfo};

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
    case_splits: &'a [CaseSplitInfo],
    selection: Option<SelectableItem>,
    filters: HypothesisFilters,
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
        case_splits: &'a [CaseSplitInfo],
        selection: Option<SelectableItem>,
        filters: HypothesisFilters,
    ) -> Self {
        Self {
            goals,
            case_splits,
            selection,
            filters,
        }
    }

    /// Render using Frame (convenience method for non-stateful usage).
    pub fn render_to_frame(&self, frame: &mut Frame, area: Rect) -> Vec<ClickRegion> {
        let mut state = GoalTreeState::default();
        frame.render_stateful_widget(
            GoalTree::new(self.goals, self.case_splits, self.selection, self.filters),
            area,
            &mut state,
        );
        state.click_regions
    }

    fn build_tree_elements(&self) -> Vec<TreeElement> {
        let mut elements = Vec::new();

        let has_case_splits = !self.case_splits.is_empty();
        let has_named_goals = self.goals.iter().any(|g| g.user_name.is_some());
        let use_tree = has_case_splits || (self.goals.len() > 1 && has_named_goals);

        if let Some(split) = self.case_splits.last() {
            elements.push(TreeElement::Label(render_case_split_label(
                split, "", false,
            )));
        }

        let root_prefix = if has_case_splits {
            tree_chars::EMPTY.to_string()
        } else {
            String::new()
        };

        if use_tree {
            self.append_goals_as_tree(&root_prefix, &mut elements);
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
            .count()
            .max(1);
        #[allow(clippy::cast_possible_truncation)]
        let hyp_height = 1 + visible_hyps as u16; // border + content
        let target_height = 3; // border + content + border
        hyp_height + target_height
    }
}

impl StatefulWidget for GoalTree<'_> {
    type State = GoalTreeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        if self.goals.is_empty() {
            Paragraph::new("No goals")
                .style(Style::new().fg(Color::DarkGray))
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

                    let goal_box =
                        GoalBox::new(&self.goals[*idx], *idx, self.selection, self.filters);

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
    let style = Style::new().fg(Color::DarkGray);
    let line = Line::from(Span::styled(prefix.to_string(), style));
    let lines: Vec<Line> = iter::repeat_n(line, area.height as usize).collect();
    Paragraph::new(lines).render(area, buf);
}

fn render_case_split_label(
    split: &CaseSplitInfo,
    prefix: &str,
    show_connector: bool,
) -> Line<'static> {
    let dim = Style::new().fg(Color::DarkGray);
    let label = split.name.as_ref().map_or_else(
        || format!("{}:", split.tactic),
        |name| format!("{}[{name}]:", split.tactic),
    );
    let mut spans = vec![Span::styled(prefix.to_string(), dim)];
    if show_connector {
        spans.push(Span::styled(tree_chars::MIDDLE, dim));
    }
    spans.push(Span::styled(label, Style::new().fg(Color::Magenta)));
    Line::from(spans)
}

fn render_case_label(prefix: &str, connector: &str, name: &str) -> Line<'static> {
    let dim = Style::new().fg(Color::DarkGray);
    Line::from(vec![
        Span::styled(prefix.to_string(), dim),
        Span::styled(connector.to_string(), dim),
        Span::styled(format!("{name}:"), Style::new().fg(Color::Magenta)),
    ])
}
