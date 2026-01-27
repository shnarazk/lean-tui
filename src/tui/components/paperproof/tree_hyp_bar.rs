//! Hypotheses bar - horizontal pills showing initial hypotheses.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::tree_colors;
use crate::lean_rpc::PaperproofHypothesis;

/// Render the hypotheses bar at the top with horizontal pills.
pub fn render_hyp_bar(frame: &mut Frame, area: Rect, hyps: &[PaperproofHypothesis]) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::new().fg(tree_colors::TACTIC_BORDER))
        .title(" HYPOTHESES ")
        .title_style(Style::new().fg(tree_colors::HYPOTHESIS_FG).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if hyps.is_empty() {
        return;
    }

    let pills: Vec<Span> = hyps
        .iter()
        .filter(|h| h.is_proof != "proof")
        .flat_map(|h| {
            vec![
                Span::styled(
                    format!(" {}: {} ", h.username, truncate(&h.type_, 20)),
                    Style::new()
                        .fg(tree_colors::DATA_HYP_FG)
                        .bg(tree_colors::DATA_HYP_BG)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ]
        })
        .collect();

    frame.render_widget(Paragraph::new(Line::from(pills)), inner);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max.saturating_sub(3)).collect::<String>())
    }
}
