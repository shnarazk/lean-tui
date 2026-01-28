//! Single goal rendered as bordered tables (`GoalBox` widget).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::border::Set,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Row, StatefulWidget, Table, Widget},
};

use super::{
    diff_text::{render_hypothesis_line, render_target_line},
    hypothesis_indices, ClickRegion, HypothesisFilters, Selection,
};
use crate::{
    lean_rpc::Goal,
    tui::widgets::{layout_metrics::LayoutMetrics, theme::Theme},
};

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

/// Widget for rendering a single goal with hypotheses and target.
pub struct GoalBox<'a> {
    goal: &'a Goal,
    goal_idx: usize,
    selection: Option<Selection>,
    filters: HypothesisFilters,
    /// Node ID for creating click region selections.
    node_id: Option<u32>,
    /// Optional border color override (e.g. from proof DAG node state).
    border_color: Option<Color>,
}

/// Mutable state for `GoalBox` that tracks click regions.
#[derive(Default)]
pub struct GoalBoxState {
    click_regions: Vec<ClickRegion>,
}

impl GoalBoxState {
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

impl<'a> GoalBox<'a> {
    pub const fn new(
        goal: &'a Goal,
        goal_idx: usize,
        selection: Option<Selection>,
        filters: HypothesisFilters,
        node_id: Option<u32>,
        border_color: Option<Color>,
    ) -> Self {
        Self {
            goal,
            goal_idx,
            selection,
            filters,
            node_id,
            border_color,
        }
    }

    fn visible_hyp_indices(&self) -> Vec<usize> {
        hypothesis_indices(self.goal.hyps.len(), self.filters.reverse_order)
            .filter(|&hyp_idx| self.filters.should_show(&self.goal.hyps[hyp_idx]))
            .collect()
    }

    fn title(&self) -> String {
        match self.goal.user_name.as_deref() {
            Some("Expected") => "Expected".to_string(),
            Some(name) => format!("case {name}"),
            None => format!("Goal {}", self.goal_idx + 1),
        }
    }

    #[allow(clippy::missing_const_for_fn)] // matches! with guard cannot be const
    fn is_hyp_selected(&self, hyp_idx: usize) -> bool {
        matches!(
            self.selection,
            Some(Selection::Hyp { hyp_idx: hi, .. }) if hi == hyp_idx
        )
    }

    #[allow(clippy::missing_const_for_fn)] // matches! with guard cannot be const
    fn is_target_selected(&self) -> bool {
        matches!(
            self.selection,
            Some(Selection::Goal { goal_idx: gi, .. }) if gi == self.goal_idx
        )
    }

    fn build_hyp_widget(&self, visible_indices: &[usize]) -> Table<'static> {
        let lines: Vec<Line<'static>> = visible_indices
            .iter()
            .map(|&hyp_idx| {
                render_hypothesis_line(
                    &self.goal.hyps[hyp_idx],
                    self.is_hyp_selected(hyp_idx),
                    self.filters,
                )
            })
            .collect();

        let text = if lines.is_empty() {
            Text::from(Line::from(Span::styled("(no hypotheses)", Theme::DIM)))
        } else {
            Text::from(lines.clone())
        };

        #[allow(clippy::cast_possible_truncation)]
        let row = Row::new(vec![Cell::from(text)]).height(lines.len().max(1) as u16);

        let mut title_style = Style::new().fg(Color::Yellow);
        if self.border_color.is_some() {
            title_style = title_style.add_modifier(Modifier::BOLD);
        }
        let title = Span::styled(format!(" {} ", self.title()), title_style);

        Table::new(vec![row], [Constraint::Fill(1)])
            .block(
                bordered_block(
                    Borders::TOP | Borders::LEFT | Borders::RIGHT,
                    self.border_color,
                )
                .title(title),
            )
            .column_spacing(0)
    }

    fn build_target_widget(&self) -> Table<'static> {
        let line = render_target_line(self.goal, self.is_target_selected());
        let row = Row::new(vec![Cell::from(line)]).height(1);

        Table::new(vec![row], [Constraint::Fill(1)])
            .block(bordered_block(Borders::ALL, self.border_color))
            .column_spacing(0)
    }
}

impl StatefulWidget for GoalBox<'_> {
    type State = GoalBoxState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        if area.height < 4 || area.width < 10 {
            // Not enough space to render
            return;
        }

        let visible_indices = self.visible_hyp_indices();

        // Calculate heights using LayoutMetrics
        let hyp_count = visible_indices.len().max(1);
        #[allow(clippy::cast_possible_truncation)]
        let hyp_content_height = hyp_count as u16 * LayoutMetrics::HYP_LINE_HEIGHT;
        let hyp_border_height = LayoutMetrics::HYP_BORDER_HEIGHT;
        let target_height = LayoutMetrics::TARGET_HEIGHT;

        // Layout: hypotheses table (flexible) | target table (fixed minimum)
        let [hyp_area, target_area] = Layout::vertical([
            Constraint::Min(hyp_content_height + hyp_border_height),
            Constraint::Length(target_height),
        ])
        .areas(area);

        // Render widgets
        Widget::render(self.build_hyp_widget(&visible_indices), hyp_area, buf);
        Widget::render(self.build_target_widget(), target_area, buf);

        // Compute click regions - track actual rendered positions
        if let Some(node_id) = self.node_id {
            track_goal_box_click_regions(
                &mut state.click_regions,
                node_id,
                self.goal_idx,
                &visible_indices,
                hyp_area,
                hyp_border_height,
                target_area,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn track_goal_box_click_regions(
    click_regions: &mut Vec<ClickRegion>,
    node_id: u32,
    goal_idx: usize,
    visible_indices: &[usize],
    hyp_area: Rect,
    hyp_border_height: u16,
    target_area: Rect,
) {
    let content_y = hyp_area.y + hyp_border_height;

    for (i, &hyp_idx) in visible_indices.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let hyp_y = content_y + (i as u16 * LayoutMetrics::HYP_LINE_HEIGHT);
        if hyp_y >= hyp_area.y + hyp_area.height {
            continue;
        }
        click_regions.push(ClickRegion {
            area: Rect::new(
                hyp_area.x,
                hyp_y,
                hyp_area.width,
                LayoutMetrics::HYP_LINE_HEIGHT,
            ),
            selection: Selection::Hyp { node_id, hyp_idx },
        });
    }

    // Validate click regions match visible rows
    debug_assert!(
        click_regions.len() <= visible_indices.len(),
        "Click regions must not exceed visible rows"
    );

    let target_y = target_area.y + 1; // +1 for top border
    if target_y < target_area.y + target_area.height {
        click_regions.push(ClickRegion {
            area: Rect::new(target_area.x, target_y, target_area.width, 1),
            selection: Selection::Goal { node_id, goal_idx },
        });
    }
}

fn bordered_block(borders: Borders, color_override: Option<Color>) -> Block<'static> {
    Block::default()
        .borders(borders)
        .border_set(BORDER)
        .border_style(Style::new().fg(color_override.unwrap_or(Theme::BORDER)))
}
