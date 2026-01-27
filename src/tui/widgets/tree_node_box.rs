//! Tree node box rendering for the deduction tree view.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::{
    tree_colors,
    tree_given_bar::{hyp_style_colors, truncate_str},
    ClickRegion, ClickRegionTracker, Selection,
};
use crate::tui_ipc::{HypothesisInfo, ProofDagNode};

/// Calculate height for a DAG node box.
pub fn node_height(node: &ProofDagNode) -> u16 {
    let has_hyps = !node.new_hypotheses.is_empty();
    3 + u16::from(has_hyps)
}

/// Get border color for a DAG node.
pub const fn node_border_color(node: &ProofDagNode) -> Color {
    if node.is_current {
        tree_colors::CURRENT_BORDER
    } else if node.is_leaf && !node.is_complete {
        tree_colors::INCOMPLETE_BORDER
    } else if node.is_leaf && node.is_complete {
        tree_colors::COMPLETED_BORDER
    } else {
        tree_colors::TACTIC_BORDER
    }
}

/// Render a single hypothesis span.
fn render_hyp_span(h: &HypothesisInfo, is_selected: bool) -> Span<'static> {
    let (fg, bg) = hyp_style_colors(h.is_proof);
    let mut style = Style::new().fg(fg).bg(bg);
    if is_selected {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    Span::styled(
        format!(" {}: {} ", h.name, truncate_str(&h.type_, 15)),
        style,
    )
}

/// Render line showing new hypotheses introduced by a node.
fn render_new_hyps_line(
    node: &ProofDagNode,
    selection: Option<Selection>,
) -> Option<Line<'static>> {
    if node.new_hypotheses.is_empty() {
        return None;
    }

    let mut hyp_spans: Vec<Span> = Vec::new();

    for (i, &hyp_idx) in node.new_hypotheses.iter().take(3).enumerate() {
        if i > 0 {
            hyp_spans.push(Span::raw(" "));
        }

        let Some(h) = node.state_after.hypotheses.get(hyp_idx) else {
            continue;
        };

        let is_selected = matches!(
            selection,
            Some(Selection::Hyp { node_id, hyp_idx: hi })
                if node_id == node.id && hi == hyp_idx
        );

        hyp_spans.push(render_hyp_span(h, is_selected));
    }

    if node.new_hypotheses.len() > 3 {
        hyp_spans.push(Span::styled(
            format!(" +{}", node.new_hypotheses.len() - 3),
            Style::new().fg(Color::DarkGray),
        ));
    }

    Some(Line::from(hyp_spans))
}

/// Render goals line for a DAG node.
fn render_goals_line(node: &ProofDagNode, selection: Option<Selection>) -> Line<'static> {
    if node.is_complete {
        return Line::from(vec![Span::styled(
            "✓ Goal completed",
            Style::new()
                .fg(tree_colors::COMPLETED_FG)
                .add_modifier(Modifier::BOLD),
        )]);
    }

    let mut spans: Vec<Span> = Vec::new();

    if node.is_leaf {
        spans.push(Span::styled(
            "⋯ ",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    for (goal_idx, g) in node.state_after.goals.iter().enumerate() {
        if goal_idx > 0 {
            spans.push(Span::styled(" │ ", Style::new().fg(Color::DarkGray)));
        }

        let is_selected = matches!(
            selection,
            Some(Selection::Goal { node_id, goal_idx: gi })
                if node_id == node.id && gi == goal_idx
        );

        let underline = if is_selected {
            Modifier::UNDERLINED
        } else {
            Modifier::empty()
        };

        let goal_type = truncate_str(&g.type_, 35);

        if !g.username.is_empty() && g.username != "[anonymous]" {
            spans.push(Span::styled(
                format!("{}: ", g.username),
                Style::new().fg(Color::Cyan).add_modifier(underline),
            ));
        }

        spans.push(Span::styled(
            format!("⊢ {goal_type}"),
            Style::new()
                .fg(tree_colors::GOAL_FG)
                .add_modifier(underline),
        ));
    }

    if spans.is_empty() && !node.is_complete {
        spans.push(Span::styled("⊢ ...", Style::new().fg(tree_colors::GOAL_FG)));
    }

    Line::from(spans)
}

/// Render a single DAG node box.
pub fn render_node_box(
    frame: &mut Frame,
    area: Rect,
    node: &ProofDagNode,
    selection: Option<Selection>,
    click_regions: &mut Vec<ClickRegion>,
    top_down: bool,
) {
    let border_color = node_border_color(node);
    let border_style = if node.is_current {
        Style::new().fg(border_color).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(border_color)
    };

    let title = if node.children.len() > 1 {
        format!(" {} [{}→] ", node.tactic.text, node.children.len())
    } else {
        format!(" {} ", node.tactic.text)
    };

    let title_fg = if node.is_current {
        Color::White
    } else {
        Color::Gray
    };
    let title_mod = if node.is_current {
        Modifier::BOLD
    } else {
        Modifier::empty()
    };
    let title_style = Style::new().fg(title_fg).add_modifier(title_mod);

    // Use arrows on borders to indicate deduction direction
    let arrow = if top_down { "▼" } else { "▲" };
    let arrow_title = Span::styled(format!(" {arrow} "), border_style);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(title, title_style))
        .title_bottom(arrow_title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 1 {
        return;
    }

    let has_hyps = !node.new_hypotheses.is_empty();
    let hyps_line = render_new_hyps_line(node, selection);
    let goals_line = render_goals_line(node, selection);

    // Order lines based on direction: top-down = hyps then goals, bottom-up = goals
    // then hyps
    let lines: Vec<Line> = if top_down {
        let mut v: Vec<Line> = hyps_line.into_iter().collect();
        v.push(goals_line);
        v
    } else {
        let mut v = vec![goals_line];
        v.extend(hyps_line);
        v
    };

    // Track click regions with Y positions based on direction
    let (hyps_y, goals_y) = if top_down {
        (inner.y, if has_hyps { inner.y + 1 } else { inner.y })
    } else {
        (if has_hyps { inner.y + 1 } else { inner.y }, inner.y)
    };

    if has_hyps && hyps_y < inner.y + inner.height {
        track_hyp_click_regions(click_regions, node, inner, hyps_y);
    }

    if goals_y < inner.y + inner.height {
        track_goal_click_regions(click_regions, node, inner, goals_y);
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

/// Track click regions for new hypotheses based on actual text widths.
fn track_hyp_click_regions(
    click_regions: &mut Vec<ClickRegion>,
    node: &ProofDagNode,
    inner: Rect,
    hyps_y: u16,
) {
    if node.new_hypotheses.is_empty() {
        return;
    }

    let mut tracker = ClickRegionTracker::new(inner.x, hyps_y, inner.width);

    for (i, &hyp_idx) in node.new_hypotheses.iter().take(3).enumerate() {
        if i > 0 {
            tracker.skip(1); // Space separator
        }

        let Some(h) = node.state_after.hypotheses.get(hyp_idx) else {
            continue;
        };

        // " {name}: {type} " (with padding)
        let type_str = truncate_str(&h.type_, 15);
        let char_count = h.name.chars().count() + type_str.chars().count() + 4;

        tracker.push(
            click_regions,
            char_count,
            Selection::Hyp {
                node_id: node.id,
                hyp_idx,
            },
        );
    }
}

/// Track click regions for goals on their line based on actual text widths.
fn track_goal_click_regions(
    click_regions: &mut Vec<ClickRegion>,
    node: &ProofDagNode,
    inner: Rect,
    goals_y: u16,
) {
    let goals = &node.state_after.goals;
    if goals.is_empty() {
        return;
    }

    let mut tracker = ClickRegionTracker::new(inner.x, goals_y, inner.width);

    // Account for leading marker if leaf node
    if node.is_leaf && !node.is_complete {
        tracker.skip(2); // "⋯ " is 2 chars wide
    }

    for (goal_idx, g) in goals.iter().enumerate() {
        if goal_idx > 0 {
            tracker.skip(3); // " │ " separator
        }

        // Calculate this goal's text width
        let username_width = if !g.username.is_empty() && g.username != "[anonymous]" {
            g.username.chars().count() + 2 // "{username}: "
        } else {
            0
        };
        let goal_type = truncate_str(&g.type_, 35);
        let char_count = username_width + goal_type.chars().count() + 2; // "⊢ {type}"

        tracker.push(
            click_regions,
            char_count,
            Selection::Goal {
                node_id: node.id,
                goal_idx,
            },
        );
    }
}
