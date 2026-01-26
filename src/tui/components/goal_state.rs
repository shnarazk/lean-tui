//! GoalState component - renders a single goal as bordered tables.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use super::diff_text::{render_hypothesis_line, render_target_line};
use crate::{
    lean_rpc::Goal,
    tui::app::{hypothesis_indices, ClickRegion, HypothesisFilters, SelectableItem},
};

/// Renders a single goal as two stacked bordered tables (hypotheses + target).
pub struct GoalState<'a> {
    goal: &'a Goal,
    goal_idx: usize,
    title: String,
    selection: Option<SelectableItem>,
    filters: HypothesisFilters,
    click_regions: Vec<ClickRegion>,
    visible_hyp_indices: Vec<usize>,
}

impl<'a> GoalState<'a> {
    pub fn new(
        goal: &'a Goal,
        goal_idx: usize,
        selection: Option<SelectableItem>,
        filters: HypothesisFilters,
    ) -> Self {
        let title = goal_header(goal, goal_idx);
        let visible_hyp_indices: Vec<usize> =
            hypothesis_indices(goal.hyps.len(), filters.reverse_order)
                .filter(|&hyp_idx| filters.should_show(&goal.hyps[hyp_idx]))
                .collect();

        Self {
            goal,
            goal_idx,
            title,
            selection,
            filters,
            click_regions: Vec::new(),
            visible_hyp_indices,
        }
    }

    /// Total height needed for this goal box.
    #[allow(dead_code)]
    pub fn height(&self) -> u16 {
        self.hyp_height() + self.target_height()
    }

    /// Height of hypothesis table (top border + hypotheses).
    fn hyp_height(&self) -> u16 {
        let hyp_count = self.visible_hyp_indices.len().max(1);
        (1 + hyp_count) as u16
    }

    /// Height of target table (border + content + border).
    const fn target_height(&self) -> u16 {
        3
    }

    /// Render the goal box at the given area.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.click_regions.clear();

        let [hyp_area, target_area] = Layout::vertical([
            Constraint::Length(self.hyp_height()),
            Constraint::Length(self.target_height()),
        ])
        .areas(area);

        frame.render_widget(self.build_hyp_table(), hyp_area);
        frame.render_widget(self.build_target_table(), target_area);
        self.compute_click_regions(area);
    }

    fn build_hyp_table(&self) -> Table<'static> {
        let hyp_lines: Vec<Line<'static>> = self
            .visible_hyp_indices
            .iter()
            .map(|&hyp_idx| {
                let hyp = &self.goal.hyps[hyp_idx];
                let is_selected = self.selection
                    == Some(SelectableItem::Hypothesis {
                        goal_idx: self.goal_idx,
                        hyp_idx,
                    });
                render_hypothesis_line(hyp, is_selected, self.filters)
            })
            .collect();

        let hyp_count = hyp_lines.len().max(1);
        let hyp_text = if hyp_lines.is_empty() {
            Text::from(Line::from(Span::styled(
                "(no hypotheses)",
                Style::new().fg(Color::DarkGray),
            )))
        } else {
            Text::from(hyp_lines)
        };

        #[allow(clippy::cast_possible_truncation)]
        let hyp_row = Row::new(vec![Cell::from(hyp_text)]).height(hyp_count as u16);

        Table::new(vec![hyp_row], [Constraint::Percentage(100)])
            .block(
                Block::default()
                    .title(Span::styled(
                        format!(" {} ", self.title),
                        Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ))
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                    .border_set(border::ROUNDED)
                    .border_style(Style::new().fg(Color::DarkGray)),
            )
            .column_spacing(0)
    }

    fn build_target_table(&self) -> Table<'static> {
        let is_target_selected = self.selection
            == Some(SelectableItem::GoalTarget {
                goal_idx: self.goal_idx,
            });
        let target_line = render_target_line(self.goal, is_target_selected);
        let target_row = Row::new(vec![Cell::from(target_line)]).height(1);

        Table::new(vec![target_row], [Constraint::Percentage(100)])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(border::ROUNDED)
                    .border_style(Style::new().fg(Color::DarkGray)),
            )
            .column_spacing(0)
    }

    fn compute_click_regions(&mut self, area: Rect) {
        let goal_idx = self.goal_idx;

        // Hypothesis click regions: after top border (line 0)
        for (i, &hyp_idx) in self.visible_hyp_indices.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let hyp_y = area.y + 1 + i as u16;
            if hyp_y < area.y + area.height {
                self.click_regions.push(ClickRegion {
                    area: Rect::new(area.x, hyp_y, area.width, 1),
                    item: SelectableItem::Hypothesis { goal_idx, hyp_idx },
                });
            }
        }

        // Target click region: after hyp_table, skip target's top border
        let target_y = area.y + self.hyp_height() + 1;
        if target_y < area.y + area.height {
            self.click_regions.push(ClickRegion {
                area: Rect::new(area.x, target_y, area.width, 1),
                item: SelectableItem::GoalTarget { goal_idx },
            });
        }
    }

    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

fn goal_header(goal: &Goal, goal_idx: usize) -> String {
    match goal.user_name.as_deref() {
        Some("Expected") => "Expected".to_string(),
        Some(name) => format!("case {name}"),
        None => format!("Goal {}", goal_idx + 1),
    }
}
