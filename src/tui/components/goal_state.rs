//! Single goal rendered as bordered tables.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::border::Set,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use super::{
    diff_text::{render_hypothesis_line, render_target_line},
    hypothesis_indices, ClickRegion, HypothesisFilters, SelectableItem,
};
use crate::lean_rpc::Goal;

const BORDER: Set = Set {
    top_left: "┌",
    top_right: "┐",
    bottom_left: "└",
    bottom_right: "┘",
    vertical_left: "│",
    vertical_right: "│",
    horizontal_top: "─",
    horizontal_bottom: "─",
};

const DIM: Color = Color::DarkGray;

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

    #[allow(dead_code)]
    pub fn height(&self) -> u16 {
        self.hyp_height() + Self::target_height()
    }

    fn hyp_height(&self) -> u16 {
        let hyp_count = self.visible_hyp_indices.len().max(1);
        #[allow(clippy::cast_possible_truncation)]
        let height = 1 + hyp_count as u16;
        height
    }

    const fn target_height() -> u16 {
        3
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.click_regions.clear();

        let [hyp_area, target_area] = Layout::vertical([
            Constraint::Length(self.hyp_height()),
            Constraint::Length(Self::target_height()),
        ])
        .areas(area);

        frame.render_widget(self.build_hyp_table(), hyp_area);
        frame.render_widget(self.build_target_table(), target_area);
        self.compute_click_regions(area);
    }

    fn build_hyp_table(&self) -> Table<'static> {
        let lines: Vec<Line<'static>> = self
            .visible_hyp_indices
            .iter()
            .map(|&hyp_idx| {
                let is_selected = self.is_hyp_selected(hyp_idx);
                render_hypothesis_line(&self.goal.hyps[hyp_idx], is_selected, self.filters)
            })
            .collect();

        let text = if lines.is_empty() {
            Text::from(Line::from(Span::styled(
                "(no hypotheses)",
                Style::new().fg(DIM),
            )))
        } else {
            Text::from(lines.clone())
        };

        #[allow(clippy::cast_possible_truncation)]
        let row = Row::new(vec![Cell::from(text)]).height(lines.len().max(1) as u16);

        let title = Span::styled(
            format!(" {} ", self.title),
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        );

        Table::new(vec![row], [Constraint::Percentage(100)])
            .block(bordered_block(Borders::TOP | Borders::LEFT | Borders::RIGHT).title(title))
            .column_spacing(0)
    }

    fn build_target_table(&self) -> Table<'static> {
        let is_selected = self.is_target_selected();
        let line = render_target_line(self.goal, is_selected);
        let row = Row::new(vec![Cell::from(line)]).height(1);

        Table::new(vec![row], [Constraint::Percentage(100)])
            .block(bordered_block(Borders::ALL))
            .column_spacing(0)
    }

    fn is_hyp_selected(&self, hyp_idx: usize) -> bool {
        self.selection
            == Some(SelectableItem::Hypothesis {
                goal_idx: self.goal_idx,
                hyp_idx,
            })
    }

    fn is_target_selected(&self) -> bool {
        self.selection
            == Some(SelectableItem::GoalTarget {
                goal_idx: self.goal_idx,
            })
    }

    fn compute_click_regions(&mut self, area: Rect) {
        let goal_idx = self.goal_idx;

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

fn bordered_block(borders: Borders) -> Block<'static> {
    Block::default()
        .borders(borders)
        .border_set(BORDER)
        .border_style(Style::new().fg(DIM))
}

fn goal_header(goal: &Goal, idx: usize) -> String {
    match goal.user_name.as_deref() {
        Some("Expected") => "Expected".to_string(),
        Some(name) => format!("case {name}"),
        None => format!("Goal {}", idx + 1),
    }
}
