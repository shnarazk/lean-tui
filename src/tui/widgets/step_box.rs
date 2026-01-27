//! Step box widget - renders a single proof step with hypotheses and goals.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget, Wrap},
};

use super::tree_colors;
use crate::{
    lean_rpc::{PaperproofHypothesis, PaperproofStep},
    tui::modes::deduction_tree::{TreeClickRegions, TreeSelection},
};

/// State for the step box widget.
pub struct StepBoxState<'a> {
    pub step: &'a PaperproofStep,
    pub step_idx: usize,
    pub is_current: bool,
    pub branch_count: usize,
    pub selection: Option<TreeSelection>,
}

/// Widget for rendering a proof step box.
pub struct StepBox<'a> {
    pub click_regions: &'a mut TreeClickRegions,
}

impl<'a> StatefulWidget for StepBox<'a> {
    type State = StepBoxState<'a>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let is_complete = state.step.goals_after.is_empty();
        let is_leaf = state.branch_count == 0;

        let border_color = step_border_color(state.is_current, is_leaf, is_complete);
        let border_style = if state.is_current {
            Style::new().fg(border_color).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(border_color)
        };

        let title = format_step_title(&state.step.tactic_string, state.branch_count);
        let title_style = Style::new()
            .fg(if state.is_current {
                Color::White
            } else {
                Color::Gray
            })
            .add_modifier(if state.is_current {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(title, title_style));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        // Render new hypotheses
        let new_hyps = get_new_hypotheses(state.step);
        if !new_hyps.is_empty() {
            lines.push(render_hyp_line(&new_hyps, state.step_idx, state.selection));
        }

        // Render goals
        lines.push(render_goal_line(
            state.step,
            state.step_idx,
            is_leaf,
            state.selection,
        ));

        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .render(inner, buf);

        // Register click region for this step box
        if !state.step.goals_after.is_empty() {
            self.click_regions.add(
                area,
                TreeSelection::StepGoal {
                    step_idx: state.step_idx,
                    goal_idx: 0,
                },
            );
        }
    }
}

const fn step_border_color(is_current: bool, is_leaf: bool, is_complete: bool) -> Color {
    if is_current {
        tree_colors::CURRENT_BORDER
    } else if is_leaf && !is_complete {
        tree_colors::INCOMPLETE_BORDER
    } else if is_leaf && is_complete {
        tree_colors::COMPLETED_BORDER
    } else {
        tree_colors::TACTIC_BORDER
    }
}

fn format_step_title(tactic: &str, branch_count: usize) -> String {
    if branch_count > 1 {
        format!(" {tactic} [{branch_count}\u{2192}] ")
    } else {
        format!(" {tactic} ")
    }
}

/// Get hypotheses that are new in this step (introduced by the tactic).
fn get_new_hypotheses(step: &PaperproofStep) -> Vec<(usize, &PaperproofHypothesis)> {
    use std::collections::HashSet;

    let before_ids: HashSet<&str> = step
        .goal_before
        .hyps
        .iter()
        .map(|h| h.id.as_str())
        .collect();

    step.goals_after
        .first()
        .map(|goal| {
            goal.hyps
                .iter()
                .enumerate()
                .filter(|(_, h)| !before_ids.contains(h.id.as_str()))
                .collect()
        })
        .unwrap_or_default()
}

fn render_hyp_line(
    new_hyps: &[(usize, &PaperproofHypothesis)],
    step_idx: usize,
    selection: Option<TreeSelection>,
) -> Line<'static> {
    let hyp_spans: Vec<Span> = new_hyps
        .iter()
        .take(3)
        .enumerate()
        .flat_map(|(i, (hyp_idx, h))| {
            let mut spans = vec![];
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            let is_selected = matches!(
                selection,
                Some(TreeSelection::StepHyp { step_idx: s, hyp_idx: hi }) if s == step_idx && hi == *hyp_idx
            );
            let (fg, bg) = if h.is_proof == "proof" {
                (tree_colors::HYPOTHESIS_FG, tree_colors::HYPOTHESIS_BG)
            } else {
                (tree_colors::DATA_HYP_FG, tree_colors::DATA_HYP_BG)
            };
            let mut style = Style::new().fg(fg).bg(bg);
            if is_selected {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            spans.push(Span::styled(
                format!(" {}: {} ", h.username, truncate(&h.type_, 15)),
                style,
            ));
            spans
        })
        .collect();

    if new_hyps.len() > 3 {
        let mut spans = hyp_spans;
        spans.push(Span::styled(
            format!(" +{}", new_hyps.len() - 3),
            Style::new().fg(Color::DarkGray),
        ));
        Line::from(spans)
    } else {
        Line::from(hyp_spans)
    }
}

fn render_goal_line(
    step: &PaperproofStep,
    step_idx: usize,
    is_leaf: bool,
    selection: Option<TreeSelection>,
) -> Line<'static> {
    if step.goals_after.is_empty() {
        return Line::from(vec![Span::styled(
            "\u{2713} Goal completed",
            Style::new()
                .fg(tree_colors::COMPLETED_FG)
                .add_modifier(Modifier::BOLD),
        )]);
    }

    let mut goals_text: Vec<Span> = Vec::new();

    if is_leaf {
        goals_text.push(Span::styled(
            "\u{22ef} ",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    for (goal_idx, g) in step.goals_after.iter().enumerate() {
        if goal_idx > 0 {
            goals_text.push(Span::styled(" \u{2502} ", Style::new().fg(Color::DarkGray)));
        }
        let is_selected = matches!(
            selection,
            Some(TreeSelection::StepGoal { step_idx: s, goal_idx: gi }) if s == step_idx && gi == goal_idx
        );
        let underline = if is_selected {
            Modifier::UNDERLINED
        } else {
            Modifier::empty()
        };
        let goal_type = truncate(&g.type_, 35);

        if let Some(name) = clean_goal_name(&g.username) {
            goals_text.push(Span::styled(
                format!("{name}: "),
                Style::new().fg(Color::Cyan).add_modifier(underline),
            ));
        }
        goals_text.push(Span::styled(
            format!("\u{22a2} {goal_type}"),
            Style::new()
                .fg(tree_colors::GOAL_FG)
                .add_modifier(underline),
        ));
    }

    Line::from(goals_text)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}...",
            s.chars().take(max.saturating_sub(3)).collect::<String>()
        )
    }
}

fn clean_goal_name(name: &str) -> Option<&str> {
    if name.is_empty() || name == "[anonymous]" {
        return None;
    }
    if name.contains("_@.") || name.contains("._hyg") {
        if let Some(prefix) = name.split("-_@.").next() {
            if !prefix.is_empty() && prefix != name {
                return Some(prefix);
            }
        }
        return None;
    }
    Some(name)
}
