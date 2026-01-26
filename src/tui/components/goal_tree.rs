//! Hierarchical goal tree with case labels.

use std::iter;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::{goal_state::GoalState, ClickRegion, HypothesisFilters, SelectableItem};
use crate::{
    lean_rpc::Goal,
    tui_ipc::{CaseSplitInfo, DefinitionInfo},
};

mod tree_chars {
    pub const MIDDLE: &str = "├── ";
    pub const LAST: &str = "╰── ";
    pub const VERTICAL: &str = "│   ";
    pub const EMPTY: &str = "    ";
}

enum TreeElement {
    Label(Line<'static>),
    Goal {
        idx: usize,
        prefix: String,
        height: u16,
    },
}

impl TreeElement {
    const fn height(&self) -> u16 {
        match self {
            Self::Label(_) => 1,
            Self::Goal { height, .. } => *height,
        }
    }
}

pub struct GoalTree<'a> {
    goals: &'a [Goal],
    definition: Option<&'a DefinitionInfo>,
    case_splits: &'a [CaseSplitInfo],
    selection: Option<SelectableItem>,
    filters: HypothesisFilters,
    click_regions: Vec<ClickRegion>,
}

impl<'a> GoalTree<'a> {
    pub const fn new(
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

        let constraints: Vec<Constraint> = elements
            .iter()
            .map(|e| Constraint::Length(e.height()))
            .chain(iter::once(Constraint::Fill(1)))
            .collect();

        let areas = Layout::vertical(constraints).split(area);

        for (element, area) in elements.iter().zip(areas.iter()) {
            match element {
                TreeElement::Label(line) => {
                    frame.render_widget(Paragraph::new(line.clone()), *area);
                }
                TreeElement::Goal { idx, prefix, .. } => {
                    self.render_goal_with_prefix(frame, *area, *idx, prefix);
                }
            }
        }
    }

    fn build_tree_elements(&self) -> Vec<TreeElement> {
        let mut elements = Vec::new();

        let show_header = !self.filters.hide_definition && self.definition.is_some();
        let has_case_splits = !self.case_splits.is_empty();
        let has_named_goals = self.goals.iter().any(|g| g.user_name.is_some());
        let use_tree = show_header || has_case_splits || (self.goals.len() > 1 && has_named_goals);

        if let Some(def) = self.definition.filter(|_| show_header) {
            elements.push(TreeElement::Label(render_definition_header(def)));
        }

        let base_prefix = if show_header { "  " } else { "" };

        if let Some(split) = self.case_splits.last() {
            elements.push(TreeElement::Label(render_case_split_label(
                split,
                base_prefix,
                show_header,
            )));
        }

        let root_prefix = if show_header || has_case_splits {
            format!("{base_prefix}{}", tree_chars::EMPTY)
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
                .map(|(idx, goal)| TreeElement::Goal {
                    idx,
                    prefix: String::new(),
                    height: self.goal_height(goal),
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
                height: self.goal_height(goal),
            });
        }
    }

    fn goal_height(&self, goal: &Goal) -> u16 {
        let visible_hyps = goal
            .hyps
            .iter()
            .filter(|h| self.filters.should_show(h))
            .count()
            .max(1);
        #[allow(clippy::cast_possible_truncation)]
        let hyp_table_height = 1 + visible_hyps as u16;
        let target_table_height = 3;
        hyp_table_height + target_table_height
    }

    fn render_goal_with_prefix(&mut self, frame: &mut Frame, area: Rect, idx: usize, prefix: &str) {
        #[allow(clippy::cast_possible_truncation)]
        let prefix_width = prefix.chars().count() as u16;
        let min_content_width = 10;

        let content_area = if prefix_width > 0 && area.width > prefix_width + min_content_width {
            let [prefix_area, goal_area] =
                Layout::horizontal([Constraint::Length(prefix_width), Constraint::Fill(1)])
                    .areas(area);

            render_vertical_prefix(frame, prefix_area, prefix);
            goal_area
        } else {
            area
        };

        let mut state = GoalState::new(&self.goals[idx], idx, self.selection, self.filters);
        state.render(frame, content_area);
        self.click_regions
            .extend(state.click_regions().iter().cloned());
    }

    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

fn render_vertical_prefix(frame: &mut Frame, area: Rect, prefix: &str) {
    let style = Style::new().fg(Color::DarkGray);
    let line = Line::from(Span::styled(prefix.to_string(), style));
    let lines: Vec<Line> = iter::repeat_n(line, area.height as usize).collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_definition_header(def: &DefinitionInfo) -> Line<'static> {
    let bold = Modifier::BOLD;
    Line::from(vec![
        Span::styled(
            def.kind.clone(),
            Style::new().fg(Color::Blue).add_modifier(bold),
        ),
        Span::raw(" "),
        Span::styled(
            def.name.clone(),
            Style::new().fg(Color::Cyan).add_modifier(bold),
        ),
        Span::raw(":"),
    ])
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
