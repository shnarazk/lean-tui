//! List of open goals widget - renders hypotheses and goals from ProofState.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, StatefulWidget, Widget},
    Frame,
};

use super::{
    diff_text::TaggedTextExt, hypothesis_indices, ClickRegion, HypothesisFilters, Selection,
};
use crate::{lean_rpc::ProofState, tui::widgets::theme::Theme};

/// Widget for rendering hypotheses and goals from ProofState.
pub struct OpenGoalList<'a> {
    state: &'a ProofState,
    selection: Option<Selection>,
    filters: HypothesisFilters,
    /// Node ID for creating click region selections.
    node_id: Option<u32>,
    /// Name of the goal the cursor's tactic is working on.
    active_goal_name: Option<&'a str>,
}

/// Mutable state for `OpenGoalList` that tracks click regions.
#[derive(Default)]
pub struct OpenGoalListState {
    click_regions: Vec<ClickRegion>,
}

impl OpenGoalListState {
    #[allow(dead_code)]
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

impl<'a> OpenGoalList<'a> {
    pub const fn new(
        state: &'a ProofState,
        selection: Option<Selection>,
        filters: HypothesisFilters,
        node_id: Option<u32>,
        active_goal_name: Option<&'a str>,
    ) -> Self {
        Self {
            state,
            selection,
            filters,
            node_id,
            active_goal_name,
        }
    }

    /// Render using Frame (convenience method for non-stateful usage).
    pub fn render_to_frame(&self, frame: &mut Frame, area: Rect) -> Vec<ClickRegion> {
        let mut render_state = OpenGoalListState::default();
        frame.render_stateful_widget(
            OpenGoalList::new(
                self.state,
                self.selection,
                self.filters,
                self.node_id,
                self.active_goal_name,
            ),
            area,
            &mut render_state,
        );
        render_state.click_regions
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

impl StatefulWidget for OpenGoalList<'_> {
    type State = OpenGoalListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        if self.state.goals.is_empty() && self.state.hypotheses.is_empty() {
            Paragraph::new("No goals")
                .style(Theme::DIM)
                .render(area, buf);
            return;
        }

        // Count visible hypotheses
        let visible_hyp_count =
            hypothesis_indices(self.state.hypotheses.len(), self.filters.reverse_order)
                .filter(|&i| self.should_show_hypothesis(i))
                .count();

        // Layout: hypotheses, divider, goals
        let hyp_height = visible_hyp_count.min(area.height.saturating_sub(3) as usize / 2);
        let constraints = vec![
            Constraint::Length(hyp_height as u16),
            Constraint::Length(1), // divider
            Constraint::Fill(1),   // goals
        ];
        let [hyp_area, div_area, goal_area] = Layout::vertical(constraints).areas(area);

        // Render hypotheses
        let mut y = hyp_area.y;
        for hyp_idx in hypothesis_indices(self.state.hypotheses.len(), self.filters.reverse_order) {
            if !self.should_show_hypothesis(hyp_idx) {
                continue;
            }
            if y >= hyp_area.bottom() {
                break;
            }

            let h = &self.state.hypotheses[hyp_idx];
            let is_selected = matches!(self.selection, Some(Selection::Hyp { hyp_idx: sel_idx, .. }) if sel_idx == hyp_idx);

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
            if let Some(nid) = self.node_id {
                state.click_regions.push(ClickRegion {
                    area: line_area,
                    selection: Selection::Hyp {
                        node_id: nid,
                        hyp_idx,
                    },
                });
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

            let is_selected = matches!(self.selection, Some(Selection::Goal { goal_idx: sel_idx, .. }) if sel_idx == goal_idx);
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

            // Format: "⊢ type" or "case name ⊢ type"
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
            if let Some(nid) = self.node_id {
                state.click_regions.push(ClickRegion {
                    area: line_area,
                    selection: Selection::Goal {
                        node_id: nid,
                        goal_idx,
                    },
                });
            }

            y += 1;
        }
    }
}
