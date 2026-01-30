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
use crate::{lean_rpc::ProofState, tui::widgets::theme::Theme};

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

        // Count visible hypotheses
        let visible_hyp_count =
            hypothesis_indices(self.state.hypotheses.len(), self.filters.reverse_order)
                .filter(|&i| self.should_show_hypothesis(i))
                .count();

        // Layout: hypotheses section, divider, goals section
        let hyp_height = visible_hyp_count.min(inner.height.saturating_sub(3) as usize / 2);
        let constraints = vec![
            Constraint::Length(hyp_height as u16),
            Constraint::Length(1), // divider
            Constraint::Fill(1),   // goals
        ];
        let [hyp_area, div_area, goal_area] = Layout::vertical(constraints).areas(inner);

        // Render hypotheses
        let selection = if self.is_current {
            self.selection
        } else {
            None
        };
        let node_id = if self.is_current { self.node_id } else { None };

        let mut y = hyp_area.y;
        for hyp_idx in hypothesis_indices(self.state.hypotheses.len(), self.filters.reverse_order) {
            if !self.should_show_hypothesis(hyp_idx) {
                continue;
            }
            if y >= hyp_area.bottom() {
                break;
            }

            let h = &self.state.hypotheses[hyp_idx];
            let is_selected = matches!(selection, Some(Selection::Hyp { hyp_idx: sel_idx, .. }) if sel_idx == hyp_idx);

            let style = if is_selected {
                Style::new().bg(Theme::SELECTION_BG)
            } else {
                Style::default()
            };

            // Format: "name : type" (with diff highlighting)
            let mut spans = vec![
                Span::styled(&h.name, style.fg(Theme::HYP_NAME)),
                Span::styled(" : ", style),
            ];
            spans.extend(h.type_.to_spans(style.fg(Theme::HYP_TYPE)));
            let line = Line::from(spans);

            let line_area = Rect::new(hyp_area.x, y, hyp_area.width, 1);
            Paragraph::new(line).render(line_area, buf);

            // Register click region
            if self.is_current {
                if let Some(nid) = node_id {
                    state.click_regions.push(ClickRegion {
                        area: line_area,
                        selection: Selection::Hyp {
                            node_id: nid,
                            hyp_idx,
                        },
                    });
                }
            }

            y += 1;
        }

        // Render divider
        let divider = "─".repeat(div_area.width as usize);
        Paragraph::new(divider)
            .style(Theme::DIM)
            .render(div_area, buf);

        // Render goals
        y = goal_area.y;
        for (goal_idx, g) in self.state.goals.iter().enumerate() {
            if y >= goal_area.bottom() {
                break;
            }

            let is_selected = matches!(selection, Some(Selection::Goal { goal_idx: sel_idx, .. }) if sel_idx == goal_idx);
            let is_active = self
                .active_goal_name
                .is_some_and(|name| g.username.as_str() == Some(name));

            let style = if is_selected {
                Style::new().bg(Theme::SELECTION_BG)
            } else {
                Style::default()
            };

            // Highlight active goal
            let target_style = if is_active {
                style
                    .fg(Theme::CURRENT_NODE_BORDER)
                    .add_modifier(Modifier::BOLD)
            } else {
                style.fg(Theme::GOAL_TYPE)
            };

            // Format: "⊢ type" or "case name ⊢ type" (with diff highlighting)
            let prefix = g
                .username
                .as_str()
                .map_or("⊢ ".to_string(), |name| format!("case {name} ⊢ "));
            let mut spans = vec![Span::styled(prefix, style)];
            spans.extend(g.type_.to_spans(target_style));
            let line = Line::from(spans);

            let line_area = Rect::new(goal_area.x, y, goal_area.width, 1);
            Paragraph::new(line).render(line_area, buf);

            // Register click region
            if self.is_current {
                if let Some(nid) = node_id {
                    state.click_regions.push(ClickRegion {
                        area: line_area,
                        selection: Selection::Goal {
                            node_id: nid,
                            goal_idx,
                        },
                    });
                }
            }

            y += 1;
        }
    }
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
