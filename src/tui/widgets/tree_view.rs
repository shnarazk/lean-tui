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
    step_box::{StepBox, StepBoxState},
    tree_builder::{build_proof_tree, ProofNode},
    tree_colors,
    tree_hyp_bar::render_hyp_bar,
};
use crate::{
    lean_rpc::{PaperproofHypothesis, PaperproofStep},
    tui::modes::deduction_tree::{TreeClickRegions, TreeSelection},
};

/// Minimum width for a branch column.
const MIN_BRANCH_WIDTH: u16 = 25;

/// Context for recursive tree rendering.
struct TreeRenderContext<'a> {
    steps: &'a [PaperproofStep],
    current_step_index: usize,
    top_down: bool,
    selection: Option<TreeSelection>,
    click_regions: &'a mut TreeClickRegions,
}

/// Render the proof tree view.
pub fn render_tree_view(
    frame: &mut Frame,
    area: Rect,
    steps: &[PaperproofStep],
    current_step_index: usize,
    top_down: bool,
    selection: Option<TreeSelection>,
    click_regions: &mut TreeClickRegions,
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

    // Hypotheses always at top
    // Top-down: [hyps, tree, theorem] - parent tactics near top, leaves near bottom
    // Bottom-up: [hyps, theorem, tree] - theorem after hyps, then leaves at top of
    // tree area
    let (hyps_area, tree_area, conclusion_area) = if top_down {
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas::<3>(area)
        .into()
    } else {
        let [h, c, t] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .areas(area);
        (h, t, c)
    };

    // Clear and populate click regions
    click_regions.regions.clear();

    render_hyp_bar(frame, hyps_area, initial_hyps, selection, click_regions);
    render_conclusion(
        frame,
        conclusion_area,
        initial_goal,
        selection,
        click_regions,
    );

    let mut ctx = TreeRenderContext {
        steps,
        current_step_index,
        top_down,
        selection,
        click_regions,
    };

    // Build tree structure
    if let Some(root) = build_proof_tree(steps) {
        render_tree_recursive(frame, tree_area, &mut ctx, &root);
    } else {
        // Fallback to flat list if tree building fails
        render_steps_flat(frame, tree_area, steps, current_step_index);
    }
}

/// Render the theorem conclusion at the bottom.
fn render_conclusion(
    frame: &mut Frame,
    area: Rect,
    goal: &str,
    selection: Option<TreeSelection>,
    click_regions: &mut TreeClickRegions,
) {
    let is_selected = matches!(selection, Some(TreeSelection::Theorem));

    // Register click region for theorem
    click_regions.add(area, TreeSelection::Theorem);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::Magenta))
        .title(Span::styled(
            " THEOREM ",
            Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut style = Style::new().fg(tree_colors::GOAL_FG);
    if is_selected {
        style = style.add_modifier(Modifier::UNDERLINED);
    }

    let goal_text = Paragraph::new(Line::from(vec![Span::styled(format!("⊢ {goal}"), style)]))
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
/// Returns (`index_in_goals_after`, hypothesis) pairs.
fn get_new_hypotheses(step: &PaperproofStep) -> Vec<(usize, &PaperproofHypothesis)> {
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

/// Recursively render the proof tree with horizontal branching.
fn render_tree_recursive(
    frame: &mut Frame,
    area: Rect,
    ctx: &mut TreeRenderContext,
    node: &ProofNode,
) {
    let step = &ctx.steps[node.step_index];
    let box_height = step_box_height(step);

    if area.height < box_height {
        return;
    }

    let is_current = node.step_index == ctx.current_step_index;

    // Layout depends on direction:
    // - Top-down: step above, children below (parent at top, leaves at bottom)
    // - Bottom-up: children above, step below (leaves at top, parent at bottom)
    let (children_area, step_area) = if node.children.is_empty() {
        // Leaf node: step positioned based on direction
        if ctx.top_down {
            let [step_area, _] =
                Layout::vertical([Constraint::Length(box_height), Constraint::Fill(1)]).areas(area);
            (None, step_area)
        } else {
            let [_, step_area] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(box_height)]).areas(area);
            (None, step_area)
        }
    } else if ctx.top_down {
        // Top-down: step above (fixed height), children below (Fill)
        let [step_area, children_area] =
            Layout::vertical([Constraint::Length(box_height), Constraint::Fill(1)]).areas(area);
        (Some(children_area), step_area)
    } else {
        // Bottom-up: children above (Fill), step below (fixed height)
        let [children_area, step_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(box_height)]).areas(area);
        (Some(children_area), step_area)
    };

    let mut state = StepBoxState {
        step,
        step_idx: node.step_index,
        is_current,
        branch_count: node.children.len(),
        selection: ctx.selection,
    };
    let widget = StepBox {
        click_regions: ctx.click_regions,
    };
    frame.render_stateful_widget(widget, step_area, &mut state);

    if let Some(children_area) = children_area {
        render_children(frame, children_area, ctx, &node.children);
    }
}

/// Render child branches, side-by-side if multiple.
fn render_children(
    frame: &mut Frame,
    area: Rect,
    ctx: &mut TreeRenderContext,
    children: &[ProofNode],
) {
    if children.len() > 1 {
        let constraints: Vec<Constraint> = children
            .iter()
            .map(|_| Constraint::Min(MIN_BRANCH_WIDTH))
            .collect();

        let branch_areas = Layout::horizontal(constraints).split(area);

        for (child, &branch_area) in children.iter().zip(branch_areas.iter()) {
            render_tree_recursive(frame, branch_area, ctx, child);
        }
    } else if let Some(child) = children.first() {
        render_tree_recursive(frame, area, ctx, child);
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

    // Flat list fallback doesn't support click regions
    let mut dummy_regions = TreeClickRegions::default();
    for (area_idx, (step_idx, step)) in visible_steps.into_iter().enumerate() {
        let is_current = step_idx == current_step_index;
        let mut state = StepBoxState {
            step,
            step_idx,
            is_current,
            branch_count: 0,
            selection: None,
        };
        let widget = StepBox {
            click_regions: &mut dummy_regions,
        };
        frame.render_stateful_widget(widget, areas[area_idx], &mut state);
    }
}
