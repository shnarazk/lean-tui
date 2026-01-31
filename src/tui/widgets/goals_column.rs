//! Goals column widget for temporal comparison views (Before/After mode).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};

use super::{
    diff_text::TaggedTextExt, hypothesis_indices, ClickRegion, HypothesisFilters, Selection,
};
use crate::{
    lean_rpc::{GoalInfo, HypothesisInfo, ProofState},
    tui::widgets::theme::Theme,
};

/// State for the goals column widget (render artifacts only).
#[derive(Default)]
pub struct GoalsColumnState {
    click_regions: Vec<ClickRegion>,
}

impl GoalsColumnState {
    /// Get the click regions computed during the last render.
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

/// Widget for rendering a column showing proof state (hypotheses + goals).
pub struct GoalsColumn<'a> {
    title: &'a str,
    state: &'a ProofState,
    filters: HypothesisFilters,
    selection: Option<Selection>,
    is_current: bool,
    node_id: Option<u32>,
    active_goal_name: Option<&'a str>,
}

impl<'a> GoalsColumn<'a> {
    pub const fn new(
        title: &'a str,
        state: &'a ProofState,
        filters: HypothesisFilters,
        selection: Option<Selection>,
        is_current: bool,
        node_id: Option<u32>,
        active_goal_name: Option<&'a str>,
    ) -> Self {
        Self {
            title,
            state,
            filters,
            selection,
            is_current,
            node_id,
            active_goal_name,
        }
    }

    fn should_show_hypothesis(&self, idx: usize) -> bool {
        let Some(h) = self.state.hypotheses.get(idx) else {
            return false;
        };
        if self.filters.hide_instances && h.is_instance {
            return false;
        }
        if self.filters.hide_inaccessible && h.is_proof {
            return false;
        }
        true
    }
}

impl StatefulWidget for GoalsColumn<'_> {
    type State = GoalsColumnState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        let block = create_border_block(self.title, self.is_current);
        let inner = block.inner(area);
        block.render(area, buf);

        if self.state.goals.is_empty() && self.state.hypotheses.is_empty() {
            render_empty_message(inner, buf, self.is_current);
            return;
        }

        let [hyp_area, div_area, goal_area] = self.compute_layout(inner);
        let node_id = self.is_current.then_some(self.node_id).flatten();
        let selection = self.is_current.then_some(self.selection).flatten();

        self.render_hypotheses(hyp_area, buf, state, selection, node_id);
        render_divider(div_area, buf);
        self.render_goals(goal_area, buf, state, selection, node_id);
    }
}

impl GoalsColumn<'_> {
    fn compute_layout(&self, inner: Rect) -> [Rect; 3] {
        let visible_hyp_count =
            hypothesis_indices(self.state.hypotheses.len(), self.filters.reverse_order)
                .filter(|&i| self.should_show_hypothesis(i))
                .count();

        let hyp_height = visible_hyp_count.min(inner.height.saturating_sub(3) as usize / 2);
        let constraints = vec![
            Constraint::Length(hyp_height as u16),
            Constraint::Length(1),
            Constraint::Fill(1),
        ];
        Layout::vertical(constraints).areas(inner)
    }

    fn render_hypotheses(
        &self,
        hyp_area: Rect,
        buf: &mut Buffer,
        state: &mut GoalsColumnState,
        selection: Option<Selection>,
        node_id: Option<u32>,
    ) {
        let visible_hyps = hypothesis_indices(self.state.hypotheses.len(), self.filters.reverse_order)
            .filter(|&i| self.should_show_hypothesis(i))
            .take(hyp_area.height as usize);

        for (row, hyp_idx) in visible_hyps.enumerate() {
            let h = &self.state.hypotheses[hyp_idx];
            let is_selected = matches!(selection, Some(Selection::Hyp { hyp_idx: sel, .. }) if sel == hyp_idx);
            let line_area = Rect::new(hyp_area.x, hyp_area.y + row as u16, hyp_area.width, 1);

            let line = render_hypothesis_line(h, is_selected);
            Paragraph::new(line).render(line_area, buf);

            if let Some(nid) = node_id {
                state.click_regions.push(ClickRegion {
                    area: line_area,
                    selection: Selection::Hyp { node_id: nid, hyp_idx },
                });
            }
        }
    }

    fn render_goals(
        &self,
        goal_area: Rect,
        buf: &mut Buffer,
        state: &mut GoalsColumnState,
        selection: Option<Selection>,
        node_id: Option<u32>,
    ) {
        let visible_goals = self.state.goals.iter().enumerate().take(goal_area.height as usize);

        for (goal_idx, g) in visible_goals {
            let is_selected = matches!(selection, Some(Selection::Goal { goal_idx: sel, .. }) if sel == goal_idx);
            let is_active = self.active_goal_name.is_some_and(|name| g.username.as_str() == Some(name));
            let line_area = Rect::new(goal_area.x, goal_area.y + goal_idx as u16, goal_area.width, 1);

            let line = render_goal_line(g, is_selected, is_active);
            Paragraph::new(line).render(line_area, buf);

            if let Some(nid) = node_id {
                state.click_regions.push(ClickRegion {
                    area: line_area,
                    selection: Selection::Goal { node_id: nid, goal_idx },
                });
            }
        }
    }
}

fn render_hypothesis_line(h: &HypothesisInfo, is_selected: bool) -> Line<'static> {
    let style = if is_selected {
        Style::new().bg(Theme::SELECTION_BG)
    } else {
        Style::default()
    };

    let mut spans = vec![
        Span::styled(h.name.clone(), style.fg(Theme::HYP_NAME)),
        Span::styled(" : ", style),
    ];
    spans.extend(h.type_.to_spans(style.fg(Theme::HYP_TYPE)));
    Line::from(spans)
}

fn render_goal_line(g: &GoalInfo, is_selected: bool, is_active: bool) -> Line<'static> {
    let style = if is_selected {
        Style::new().bg(Theme::SELECTION_BG)
    } else {
        Style::default()
    };

    let target_style = if is_active {
        style.fg(Theme::CURRENT_NODE_BORDER).add_modifier(Modifier::BOLD)
    } else {
        style.fg(Theme::GOAL_TYPE)
    };

    let prefix = g.username.as_str().map_or_else(
        || "⊢ ".to_string(),
        |name| format!("case {name} ⊢ "),
    );

    let mut spans = vec![Span::styled(prefix, style)];
    spans.extend(g.type_.to_spans(target_style));
    Line::from(spans)
}

fn render_divider(div_area: Rect, buf: &mut Buffer) {
    let divider = "─".repeat(div_area.width as usize);
    Paragraph::new(divider).style(Theme::DIM).render(div_area, buf);
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
