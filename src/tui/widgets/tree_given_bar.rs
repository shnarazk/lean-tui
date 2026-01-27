//! Hypothesis bar and theorem conclusion for tree view.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::{tree_colors, ClickRegion, Selection};
use crate::tui_ipc::HypothesisInfo;

/// Get hypothesis style colors based on proof status.
pub const fn hyp_style_colors(is_proof: bool) -> (Color, Color) {
    if is_proof {
        (tree_colors::HYPOTHESIS_FG, tree_colors::HYPOTHESIS_BG)
    } else {
        (tree_colors::DATA_HYP_FG, tree_colors::DATA_HYP_BG)
    }
}

/// Truncate a string to max length.
pub fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}...",
            s.chars().take(max.saturating_sub(3)).collect::<String>()
        )
    }
}

/// Render hypotheses bar from DAG initial state.
pub fn render_given_bar(
    frame: &mut Frame,
    area: Rect,
    hyps: &[HypothesisInfo],
    selection: Option<Selection>,
    click_regions: &mut Vec<ClickRegion>,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(tree_colors::HYPOTHESIS_FG))
        .title(Span::styled(
            " Given ",
            Style::new()
                .fg(tree_colors::HYPOTHESIS_FG)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 2 || inner.height < 1 {
        return;
    }

    let visible_hyps: Vec<_> = hyps
        .iter()
        .enumerate()
        .filter(|(_, h)| !h.is_proof)
        .collect();

    if visible_hyps.is_empty() {
        frame.render_widget(
            Paragraph::new("(no hypotheses)").style(Style::new().fg(Color::DarkGray)),
            inner,
        );
        return;
    }

    let hyp_spans: Vec<Span> = visible_hyps
        .iter()
        .take(5)
        .enumerate()
        .flat_map(|(i, (hyp_idx, h))| {
            let mut spans = vec![];
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            let is_selected = matches!(
                selection,
                Some(Selection::InitialHyp { hyp_idx: hi }) if hi == *hyp_idx
            );

            let (fg, bg) = hyp_style_colors(h.is_proof);
            let mut style = Style::new().fg(fg).bg(bg);
            if is_selected {
                style = style.add_modifier(Modifier::UNDERLINED);
            }

            click_regions.push(ClickRegion {
                area: Rect::new(inner.x, inner.y, inner.width, 1),
                selection: Selection::InitialHyp { hyp_idx: *hyp_idx },
            });

            let truncated_type = truncate_str(&h.type_, 20);
            spans.push(Span::styled(
                format!(" {}: {} ", h.name, truncated_type),
                style,
            ));
            spans
        })
        .collect();

    let line = if visible_hyps.len() > 5 {
        let mut spans = hyp_spans;
        spans.push(Span::styled(
            format!(" +{}", visible_hyps.len() - 5),
            Style::new().fg(Color::DarkGray),
        ));
        Line::from(spans)
    } else {
        Line::from(hyp_spans)
    };

    frame.render_widget(Paragraph::new(line), inner);
}

/// Render the theorem conclusion.
pub fn render_conclusion(
    frame: &mut Frame,
    area: Rect,
    goal: &str,
    selection: Option<Selection>,
    click_regions: &mut Vec<ClickRegion>,
) {
    let is_selected = matches!(selection, Some(Selection::Theorem));

    click_regions.push(ClickRegion {
        area,
        selection: Selection::Theorem,
    });

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
