//! GoalTree component - renders goals in a tree structure with case names.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::goal_state::GoalState;
use crate::{
    lean_rpc::Goal,
    tui::app::{ClickRegion, HypothesisFilters, SelectableItem},
    tui_ipc::{CaseSplitInfo, DefinitionInfo},
};

/// Box-drawing characters for tree visualization.
mod tree_chars {
    pub const MIDDLE: &str = "├── ";
    pub const LAST: &str = "└── ";
    pub const VERTICAL: &str = "│   ";
    pub const EMPTY: &str = "    ";
}

/// A tree element: either a label line or a goal box.
enum TreeElement {
    Label {
        line: Line<'static>,
        height: u16,
    },
    Goal {
        goal_idx: usize,
        prefix: String,
        height: u16,
    },
}

/// GoalTree renders a hierarchical view of goals with tree prefixes.
pub struct GoalTree<'a> {
    goals: &'a [Goal],
    definition: Option<&'a DefinitionInfo>,
    case_splits: &'a [CaseSplitInfo],
    selection: Option<SelectableItem>,
    filters: HypothesisFilters,
    click_regions: Vec<ClickRegion>,
}

impl<'a> GoalTree<'a> {
    pub fn new(
        goals: &'a [Goal],
        definition: Option<&'a DefinitionInfo>,
        case_splits: &'a [CaseSplitInfo],
        selection: Option<SelectableItem>,
        filters: HypothesisFilters,
    ) -> Self {
        Self {
            goals,
            definition,
            case_splits,
            selection,
            filters,
            click_regions: Vec::new(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.click_regions.clear();

        if self.goals.is_empty() {
            frame.render_widget(
                Paragraph::new("No goals").style(Style::new().fg(Color::DarkGray)),
                area,
            );
            return;
        }

        // Build tree elements
        let elements = self.build_tree_elements();

        // Build layout constraints
        let constraints: Vec<Constraint> = elements
            .iter()
            .map(|e| match e {
                TreeElement::Label { height, .. } => Constraint::Length(*height),
                TreeElement::Goal { height, .. } => Constraint::Length(*height),
            })
            .chain(std::iter::once(Constraint::Fill(1)))
            .collect();

        let areas = Layout::vertical(constraints).split(area);

        // Render each element
        for (i, element) in elements.iter().enumerate() {
            if i >= areas.len() {
                break;
            }

            match element {
                TreeElement::Label { line, .. } => {
                    frame.render_widget(Paragraph::new(line.clone()), areas[i]);
                }
                TreeElement::Goal {
                    goal_idx, prefix, ..
                } => {
                    self.render_goal_with_prefix(frame, areas[i], *goal_idx, prefix);
                }
            }
        }
    }

    fn build_tree_elements(&self) -> Vec<TreeElement> {
        let mut elements = Vec::new();

        // If definition display is hidden, use flat rendering
        if self.filters.hide_definition {
            self.add_goals_flat(&mut elements);
            return elements;
        }

        // Definition header
        if let Some(def) = self.definition {
            elements.push(TreeElement::Label {
                line: render_definition_header(def),
                height: 1,
            });
        }

        let has_header = self.definition.is_some();
        let base_prefix = if has_header { "  " } else { "" };

        // Case split label
        if let Some(split) = self.case_splits.last() {
            elements.push(TreeElement::Label {
                line: render_case_split_label(split, base_prefix),
                height: 1,
            });
        }

        // Determine if we should use tree structure
        let has_case_names = self.goals.iter().any(|g| g.user_name.is_some());
        let use_tree = has_header || (self.goals.len() > 1 && has_case_names);

        let root_prefix = if has_header || !self.case_splits.is_empty() {
            format!("{base_prefix}{}", tree_chars::EMPTY)
        } else {
            String::new()
        };

        if use_tree {
            self.add_goals_with_tree(&root_prefix, &mut elements);
        } else {
            self.add_goals_flat(&mut elements);
        }

        elements
    }

    fn add_goals_flat(&self, elements: &mut Vec<TreeElement>) {
        for (goal_idx, goal) in self.goals.iter().enumerate() {
            let height = self.compute_goal_height(goal);
            elements.push(TreeElement::Goal {
                goal_idx,
                prefix: String::new(),
                height,
            });
        }
    }

    fn add_goals_with_tree(&self, base_prefix: &str, elements: &mut Vec<TreeElement>) {
        let goal_count = self.goals.len();

        for (i, goal) in self.goals.iter().enumerate() {
            let is_last = i == goal_count - 1;
            let connector = if is_last {
                tree_chars::LAST
            } else {
                tree_chars::MIDDLE
            };
            let continuation = if is_last {
                tree_chars::EMPTY
            } else {
                tree_chars::VERTICAL
            };

            // Add case label if present
            if let Some(case_name) = &goal.user_name {
                elements.push(TreeElement::Label {
                    line: Line::from(vec![
                        Span::styled(base_prefix.to_string(), Style::new().fg(Color::DarkGray)),
                        Span::styled(connector, Style::new().fg(Color::DarkGray)),
                        Span::styled(format!("{case_name}:"), Style::new().fg(Color::Magenta)),
                    ]),
                    height: 1,
                });
            }

            let prefix = format!("{base_prefix}{continuation}");
            let height = self.compute_goal_height(goal);
            elements.push(TreeElement::Goal {
                goal_idx: i,
                prefix,
                height,
            });
        }
    }

    fn compute_goal_height(&self, goal: &Goal) -> u16 {
        let visible_hyp_count = goal
            .hyps
            .iter()
            .filter(|h| self.filters.should_show(h))
            .count()
            .max(1);
        let hyp_height = (1 + visible_hyp_count) as u16;
        let target_height = 3u16;
        hyp_height + target_height
    }

    fn render_goal_with_prefix(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        goal_idx: usize,
        prefix: &str,
    ) {
        let goal = &self.goals[goal_idx];
        let prefix_width = prefix.chars().count() as u16;

        let content_area = if prefix_width > 0 && area.width > prefix_width + 10 {
            let [p_area, c_area] =
                Layout::horizontal([Constraint::Length(prefix_width), Constraint::Fill(1)])
                    .areas(area);

            // Render tree prefix lines
            let prefix_line = Line::from(Span::styled(
                prefix.to_string(),
                Style::new().fg(Color::DarkGray),
            ));
            let prefix_lines: Vec<Line> = (0..area.height).map(|_| prefix_line.clone()).collect();
            frame.render_widget(Paragraph::new(prefix_lines), p_area);

            c_area
        } else {
            area
        };

        // Create and render goal state component
        let mut goal_state = GoalState::new(goal, goal_idx, self.selection, self.filters);
        goal_state.render(frame, content_area);

        // Collect click regions from goal state
        self.click_regions
            .extend(goal_state.click_regions().iter().cloned());
    }

    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

fn render_definition_header(definition: &DefinitionInfo) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            definition.kind.clone(),
            Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            definition.name.clone(),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(":"),
    ])
}

fn render_case_split_label(split: &CaseSplitInfo, prefix: &str) -> Line<'static> {
    let label = split.name.as_ref().map_or_else(
        || format!("{}:", split.tactic),
        |name| format!("{}[{name}]:", split.tactic),
    );
    Line::from(vec![
        Span::styled(prefix.to_string(), Style::new().fg(Color::DarkGray)),
        Span::styled(tree_chars::MIDDLE, Style::new().fg(Color::DarkGray)),
        Span::styled(label, Style::new().fg(Color::Magenta)),
    ])
}
