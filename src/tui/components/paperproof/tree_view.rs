//! Tree view - main component that composes the proof tree visualization.

use std::collections::HashSet;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::{
    tree_builder::{build_proof_tree, ProofNode},
    tree_colors,
    tree_hyp_bar::render_hyp_bar,
};
use crate::lean_rpc::{PaperproofHypothesis, PaperproofStep};

/// Minimum width for a branch column.
const MIN_BRANCH_WIDTH: u16 = 25;

/// Render the proof tree view.
pub fn render_tree_view(
    frame: &mut Frame,
    area: Rect,
    steps: &[PaperproofStep],
    current_step_index: usize,
) {
    if steps.is_empty() {
        frame.render_widget(
            Paragraph::new("No proof steps").style(Style::new().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let initial_hyps = &steps[0].goal_before.hyps;
    let initial_goal = &steps[0].goal_before.type_;

    let [hyps_area, tree_area, conclusion_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(3),
    ])
    .areas(area);

    render_hyp_bar(frame, hyps_area, initial_hyps);

    // Build tree structure
    if let Some(root) = build_proof_tree(steps) {
        render_tree_recursive(frame, tree_area, steps, &root, current_step_index);
    } else {
        // Fallback to flat list if tree building fails
        render_steps_flat(frame, tree_area, steps, current_step_index);
    }

    // Render theorem conclusion at the bottom
    render_conclusion(frame, conclusion_area, initial_goal);
}

/// Render the theorem conclusion at the bottom.
fn render_conclusion(frame: &mut Frame, area: Rect, goal: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::Magenta))
        .title(Span::styled(
            " THEOREM ",
            Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let goal_text = Paragraph::new(Line::from(vec![Span::styled(
        format!("⊢ {goal}"),
        Style::new().fg(Color::White),
    )]))
    .wrap(Wrap { trim: true });

    frame.render_widget(goal_text, inner);
}

/// Calculate the height needed for a step box.
fn step_box_height(step: &PaperproofStep) -> u16 {
    let new_hyps = get_new_hypotheses(step);
    // 2 for borders + 1 for goal + 1 if there are hypotheses
    let has_hyps = u16::from(!new_hyps.is_empty());
    3 + has_hyps
}

/// Get hypotheses that are new in this step (introduced by the tactic).
fn get_new_hypotheses(step: &PaperproofStep) -> Vec<&PaperproofHypothesis> {
    let before_ids: HashSet<&str> = step.goal_before.hyps.iter().map(|h| h.id.as_str()).collect();

    step.goals_after
        .first()
        .map(|goal| {
            goal.hyps
                .iter()
                .filter(|h| !before_ids.contains(h.id.as_str()))
                .collect()
        })
        .unwrap_or_default()
}

/// Recursively render the proof tree with horizontal branching.
/// Uses bottom-up ordering to match Paperproof: children appear above, parent below.
fn render_tree_recursive(
    frame: &mut Frame,
    area: Rect,
    steps: &[PaperproofStep],
    node: &ProofNode,
    current_step_index: usize,
) {
    let step = &steps[node.step_index];
    let box_height = step_box_height(step);

    if area.height < box_height {
        return;
    }

    let is_current = node.step_index == current_step_index;

    // REVERSED: children area FIRST (top), step area SECOND (bottom)
    // This matches Paperproof's bottom-up visual flow where the conclusion
    // appears at the bottom and leaf tactics appear at the top.
    let (children_area, step_area) = if node.children.is_empty() {
        // Leaf node: step at top, remaining space unused
        let [step_area, _] =
            Layout::vertical([Constraint::Length(box_height), Constraint::Fill(1)]).areas(area);
        (None, step_area)
    } else {
        // Non-leaf: children above (Fill), this step below (fixed height)
        let [children_area, step_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(box_height)]).areas(area);
        (Some(children_area), step_area)
    };

    // Render children FIRST (they appear above)
    if let Some(children_area) = children_area {
        render_children(frame, children_area, steps, &node.children, current_step_index);
    }

    // Render this step SECOND (appears below children)
    render_step_box(frame, step_area, step, is_current, node.children.len());
}

/// Render child branches, side-by-side if multiple.
fn render_children(
    frame: &mut Frame,
    area: Rect,
    steps: &[PaperproofStep],
    children: &[ProofNode],
    current_step_index: usize,
) {
    if children.len() > 1 {
        // Multiple branches: render side-by-side with flexible widths
        let constraints: Vec<Constraint> = children.iter().map(|_| Constraint::Min(MIN_BRANCH_WIDTH)).collect();

        let branch_areas = Layout::horizontal(constraints).split(area);

        for (child, &branch_area) in children.iter().zip(branch_areas.iter()) {
            render_tree_recursive(frame, branch_area, steps, child, current_step_index);
        }
    } else if let Some(child) = children.first() {
        // Single child: continue vertically
        render_tree_recursive(frame, area, steps, child, current_step_index);
    }
}

/// Render a step box with hypotheses and goal.
fn render_step_box(
    frame: &mut Frame,
    area: Rect,
    step: &PaperproofStep,
    is_current: bool,
    branch_count: usize,
) {
    let is_complete = step.goals_after.is_empty();
    let is_leaf = branch_count == 0;

    // Border color priority: current > incomplete leaf > complete leaf > default
    let border_color = if is_current {
        tree_colors::CURRENT_BORDER // Cyan - highest priority
    } else if is_leaf && !is_complete {
        tree_colors::INCOMPLETE_BORDER // Yellow - needs attention
    } else if is_leaf && is_complete {
        tree_colors::COMPLETED_BORDER // Green - done
    } else {
        tree_colors::TACTIC_BORDER // Dark gray - internal nodes
    };

    let border_style = if is_current {
        Style::new().fg(border_color).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(border_color)
    };

    // Add branch indicator to title if this creates branches
    let title = if branch_count > 1 {
        format!(" {} [{}→] ", step.tactic_string, branch_count)
    } else {
        format!(" {} ", step.tactic_string)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            title,
            Style::new()
                .fg(if is_current { Color::White } else { Color::Gray })
                .add_modifier(if is_current {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 1 {
        return;
    }

    // Get new hypotheses introduced by this step
    let new_hyps = get_new_hypotheses(step);

    // Build content lines
    let mut lines: Vec<Line> = Vec::new();

    // Add hypothesis line if there are new hypotheses
    if !new_hyps.is_empty() {
        let hyp_spans: Vec<Span> = new_hyps
            .iter()
            .take(3) // Limit to 3 hypotheses per line
            .enumerate()
            .flat_map(|(i, h)| {
                let mut spans = vec![];
                if i > 0 {
                    spans.push(Span::raw(" "));
                }
                // Render as colored pill: [name: type]
                spans.push(Span::styled(
                    format!(" {}: {} ", h.username, truncate(&h.type_, 15)),
                    Style::new()
                        .fg(tree_colors::HYPOTHESIS_FG)
                        .bg(tree_colors::HYPOTHESIS_BG),
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
            lines.push(Line::from(spans));
        } else {
            lines.push(Line::from(hyp_spans));
        }
    }

    lines.push(render_goal_line(step, is_leaf));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

/// Render the goal line for a step box.
fn render_goal_line(step: &PaperproofStep, is_leaf: bool) -> Line<'static> {
    if step.goals_after.is_empty() {
        Line::from(vec![Span::styled(
            "✓ Goal completed",
            Style::new()
                .fg(tree_colors::COMPLETED_FG)
                .add_modifier(Modifier::BOLD),
        )])
    } else {
        let mut goals_text: Vec<Span> = Vec::new();

        // Add ellipsis indicator for incomplete leaf nodes
        if is_leaf {
            goals_text.push(Span::styled(
                "⋯ ",
                Style::new()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        for (i, g) in step.goals_after.iter().enumerate() {
            if i > 0 {
                goals_text.push(Span::styled(" │ ", Style::new().fg(Color::DarkGray)));
            }
            let goal_type = truncate(&g.type_, 35);
            if let Some(name) = clean_goal_name(&g.username) {
                goals_text.push(Span::styled(format!("{name}: "), Style::new().fg(Color::Cyan)));
            }
            goals_text.push(Span::styled(
                format!("⊢ {goal_type}"),
                Style::new().fg(tree_colors::GOAL_FG),
            ));
        }
        Line::from(goals_text)
    }
}

/// Fallback: render steps in a flat vertical list.
fn render_steps_flat(
    frame: &mut Frame,
    area: Rect,
    steps: &[PaperproofStep],
    current_step_index: usize,
) {
    // Calculate heights for visible steps
    let mut total_height = 0u16;
    let mut visible_range = (0, steps.len());

    // Find a window around current_step_index that fits
    for (i, step) in steps.iter().enumerate() {
        let h = step_box_height(step);
        if total_height + h > area.height {
            if i <= current_step_index {
                // Need to start later
                visible_range.0 = i.saturating_sub(1);
                total_height = 0;
            } else {
                visible_range.1 = i;
                break;
            }
        }
        total_height += h;
    }

    let visible_steps: Vec<_> = steps
        .iter()
        .enumerate()
        .skip(visible_range.0)
        .take(visible_range.1 - visible_range.0)
        .collect();

    if visible_steps.is_empty() {
        return;
    }

    let constraints: Vec<Constraint> = visible_steps
        .iter()
        .map(|(_, step)| Constraint::Length(step_box_height(step)))
        .collect();

    let areas = Layout::vertical(constraints).split(area);

    for (area_idx, (step_idx, step)) in visible_steps.into_iter().enumerate() {
        let is_current = step_idx == current_step_index;
        render_step_box(frame, areas[area_idx], step, is_current, 0);
    }
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

/// Clean up a goal username, returning None if it should be hidden.
///
/// Filters out internal Lean identifiers like:
/// - `[anonymous]` - anonymous goals
/// - `pos-_@.test.123._hygCtx._hyg.31` - hygiene-mangled names
fn clean_goal_name(name: &str) -> Option<&str> {
    if name.is_empty() || name == "[anonymous]" {
        return None;
    }

    // Check for hygiene-mangled names (contain `_@.` or `._hyg`)
    if name.contains("_@.") || name.contains("._hyg") {
        // Try to extract the meaningful prefix (e.g., "pos" from "pos-_@.test...")
        if let Some(prefix) = name.split("-_@.").next() {
            if !prefix.is_empty() && prefix != name {
                return Some(prefix);
            }
        }
        return None;
    }

    Some(name)
}
