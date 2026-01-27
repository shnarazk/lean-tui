//! Hypotheses bar - horizontal pills showing initial hypotheses.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::tree_colors;
use crate::{
    lean_rpc::PaperproofHypothesis,
    tui::modes::deduction_tree::{TreeClickRegions, TreeSelection},
};

/// Render the hypotheses bar at the top with horizontal pills.
pub fn render_hyp_bar(
    frame: &mut Frame,
    area: Rect,
    hyps: &[PaperproofHypothesis],
    selection: Option<TreeSelection>,
    click_regions: &mut TreeClickRegions,
) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::new().fg(tree_colors::TACTIC_BORDER))
        .title(" HYPOTHESES ")
        .title_style(
            Style::new()
                .fg(tree_colors::HYPOTHESIS_FG)
                .add_modifier(Modifier::BOLD),
        );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if hyps.is_empty() {
        return;
    }

    // Register click regions for initial hypotheses
    // Since pills are inline, we register the whole area for the first visible hyp
    // (clicking anywhere in the bar selects first hyp)
    if let Some((first_idx, _)) = hyps.iter().enumerate().find(|(_, h)| h.is_proof != "proof") {
        click_regions.add(inner, TreeSelection::InitialHyp { hyp_idx: first_idx });
    }

    // Track actual index among non-proof hypotheses for selection matching
    let pills: Vec<Span> = hyps
        .iter()
        .enumerate()
        .filter(|(_, h)| h.is_proof != "proof")
        .flat_map(|(idx, h)| {
            let is_selected =
                matches!(selection, Some(TreeSelection::InitialHyp { hyp_idx }) if hyp_idx == idx);
            let mut style = Style::new()
                .fg(tree_colors::DATA_HYP_FG)
                .bg(tree_colors::DATA_HYP_BG)
                .add_modifier(Modifier::BOLD);
            if is_selected {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            vec![
                Span::styled(
                    format!(" {}: {} ", h.username, truncate(&h.type_, 20)),
                    style,
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
        format!(
            "{}...",
            s.chars().take(max.saturating_sub(3)).collect::<String>()
        )
    }
}
