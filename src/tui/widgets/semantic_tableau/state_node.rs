//! State node widget - renders a single proof state node in the semantic
//! tableau.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget, Wrap},
};

use super::{
    given_pane::{hyp_style_colors, truncate_str},
    ClickRegion, Selection,
};
use crate::{lean_rpc::Goal, tui::widgets::theme::Theme, tui_ipc::ProofDagNode};

/// State for a single state node widget.
#[derive(Default)]
pub struct StateNodeState {
    /// Click regions generated during rendering.
    pub click_regions: Vec<ClickRegion>,
}

/// A single proof state node widget.
pub struct StateNode<'a> {
    node: &'a ProofDagNode,
    is_current: bool,
    selection: Option<Selection>,
    top_down: bool,
    /// Override goals from LSP (for current node).
    override_goals: Option<&'a [Goal]>,
}

impl<'a> StateNode<'a> {
    pub const fn new(
        node: &'a ProofDagNode,
        is_current: bool,
        selection: Option<Selection>,
        top_down: bool,
        override_goals: Option<&'a [Goal]>,
    ) -> Self {
        Self {
            node,
            is_current,
            selection,
            top_down,
            override_goals,
        }
    }

    /// Determine if the node should show as complete.
    fn is_effective_complete(&self) -> bool {
        self.override_goals
            .map_or_else(|| self.node.is_complete(), <[Goal]>::is_empty)
    }

    /// Get the border color for this node.
    fn border_color(&self) -> Color {
        let effective_complete = self.is_effective_complete();
        if self.is_current {
            Theme::CURRENT_NODE_BORDER
        } else if self.node.has_unsolved_spawned_goals {
            // Node has inline proofs (spawned goals) that were never solved
            Theme::INCOMPLETE_NODE_BORDER
        } else if self.node.is_leaf() && !effective_complete {
            Theme::INCOMPLETE_NODE_BORDER
        } else if self.node.is_leaf() && effective_complete {
            Theme::COMPLETED_NODE_BORDER
        } else {
            Theme::TACTIC_BORDER
        }
    }

    /// Build the title string for the node.
    fn build_title(&self) -> String {
        if self.node.children.len() > 1 {
            format!(
                " {} [{}→] ",
                self.node.tactic.text,
                self.node.children.len()
            )
        } else {
            format!(" {} ", self.node.tactic.text)
        }
    }

    /// Build the hypothesis line (horizontal layout).
    fn build_hyps_line(&self) -> Option<Line<'static>> {
        if self.node.new_hypotheses.is_empty() {
            return None;
        }

        let spans: Vec<Span> = self
            .node
            .new_hypotheses
            .iter()
            .enumerate()
            .filter_map(|(i, &hyp_idx)| {
                let h = self.node.state_after.hypotheses.get(hyp_idx)?;
                let selected = matches!(
                    self.selection,
                    Some(Selection::Hyp { node_id, hyp_idx: hi }) if node_id == self.node.id && hi == hyp_idx
                );
                let (fg, bg) = hyp_style_colors(h.is_proof);
                let style = Style::new()
                    .fg(fg)
                    .bg(bg)
                    .add_modifier(if selected { Modifier::UNDERLINED } else { Modifier::empty() });
                let text = format!(" {}: {} ", h.name, truncate_str(&h.type_, 20));
                let mut result = Vec::new();
                if i > 0 {
                    result.push(Span::raw(" "));
                }
                result.push(Span::styled(text, style));
                Some(result)
            })
            .flatten()
            .collect();

        Some(Line::from(spans))
    }

    /// Build the goals line.
    fn build_goals_line(&self) -> Line<'static> {
        if self.is_effective_complete() {
            return Line::from(vec![Span::styled(
                "✓ Goal completed",
                Style::new()
                    .fg(Theme::COMPLETED_GOAL_FG)
                    .add_modifier(Modifier::BOLD),
            )]);
        }

        let mut spans: Vec<Span> = Vec::new();
        if self.node.is_leaf() {
            spans.push(Span::styled(
                "⋯ ",
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        }

        // Use override goals if provided
        if let Some(goals) = self.override_goals {
            self.append_goal_spans_from_lsp(&mut spans, goals);
        } else {
            self.append_goal_spans_from_node(&mut spans);
        }

        if spans.is_empty() {
            spans.push(Span::styled("⊢ ...", Style::new().fg(Theme::GOAL_FG)));
        }
        Line::from(spans)
    }

    /// Append goal spans from LSP goals.
    fn append_goal_spans_from_lsp(&self, spans: &mut Vec<Span<'static>>, goals: &[Goal]) {
        for (goal_idx, g) in goals.iter().enumerate() {
            if goal_idx > 0 {
                spans.push(Span::styled(" │ ", Style::new().fg(Color::DarkGray)));
            }

            let selected = matches!(
                self.selection,
                Some(Selection::Goal { node_id, goal_idx: gi }) if node_id == self.node.id && gi == goal_idx
            );
            let underline = if selected {
                Modifier::UNDERLINED
            } else {
                Modifier::empty()
            };
            let goal_type = truncate_str(&g.target.to_plain_text(), 35);

            let show_username = g
                .user_name
                .as_ref()
                .is_some_and(|u| !u.is_empty() && u != "[anonymous]");
            if show_username {
                let username = g.user_name.as_ref().unwrap();
                spans.push(Span::styled(
                    format!("{username}: "),
                    Style::new().fg(Color::Cyan).add_modifier(underline),
                ));
            }
            spans.push(Span::styled(
                format!("⊢ {goal_type}"),
                Style::new().fg(Theme::GOAL_FG).add_modifier(underline),
            ));
        }
    }

    /// Append goal spans from node's `state_after`.
    fn append_goal_spans_from_node(&self, spans: &mut Vec<Span<'static>>) {
        for (goal_idx, g) in self.node.state_after.goals.iter().enumerate() {
            if goal_idx > 0 {
                spans.push(Span::styled(" │ ", Style::new().fg(Color::DarkGray)));
            }

            let selected = matches!(
                self.selection,
                Some(Selection::Goal { node_id, goal_idx: gi }) if node_id == self.node.id && gi == goal_idx
            );
            let underline = if selected {
                Modifier::UNDERLINED
            } else {
                Modifier::empty()
            };
            let goal_type = truncate_str(&g.type_, 35);

            if let Some(name) = g.username.as_str() {
                spans.push(Span::styled(
                    format!("{name}: "),
                    Style::new().fg(Color::Cyan).add_modifier(underline),
                ));
            }
            spans.push(Span::styled(
                format!("⊢ {goal_type}"),
                Style::new().fg(Theme::GOAL_FG).add_modifier(underline),
            ));
        }
    }

    /// Build click regions for goals and hypotheses.
    fn build_click_regions(&self, inner: Rect) -> Vec<ClickRegion> {
        let mut regions = Vec::new();
        let has_hyps = !self.node.new_hypotheses.is_empty();

        let (hyps_y, goals_y) = if self.top_down {
            (inner.y, if has_hyps { inner.y + 1 } else { inner.y })
        } else {
            (if has_hyps { inner.y + 1 } else { inner.y }, inner.y)
        };

        // Goal click regions
        let goal_count = self
            .override_goals
            .map_or(self.node.state_after.goals.len(), <[Goal]>::len);
        for goal_idx in 0..goal_count {
            regions.push(ClickRegion {
                area: Rect::new(inner.x, goals_y, inner.width, 1),
                selection: Selection::Goal {
                    node_id: self.node.id,
                    goal_idx,
                },
            });
        }

        // Hypothesis click regions
        if has_hyps {
            for &hyp_idx in &self.node.new_hypotheses {
                regions.push(ClickRegion {
                    area: Rect::new(inner.x, hyps_y, inner.width, 1),
                    selection: Selection::Hyp {
                        node_id: self.node.id,
                        hyp_idx,
                    },
                });
            }
        }

        regions
    }
}

impl StatefulWidget for StateNode<'_> {
    type State = StateNodeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        let border_color = self.border_color();
        let border_style = Style::new()
            .fg(border_color)
            .add_modifier(if self.is_current {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });

        let title = self.build_title();
        let title_style = Style::new()
            .fg(if self.is_current {
                Color::White
            } else {
                Color::Gray
            })
            .add_modifier(if self.is_current {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });

        let arrow = if self.top_down { "▼" } else { "▲" };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(title, title_style))
            .title_bottom(Span::styled(format!(" {arrow} "), border_style));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 {
            return;
        }

        // Build content lines
        let hyps_line = self.build_hyps_line();
        let goals_line = self.build_goals_line();

        let lines: Vec<Line> = match (self.top_down, hyps_line) {
            (true, Some(h)) => vec![h, goals_line],
            (false, Some(h)) => vec![goals_line, h],
            (_, None) => vec![goals_line],
        };

        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .render(inner, buf);

        // Build click regions
        state.click_regions = self.build_click_regions(inner);
    }
}
