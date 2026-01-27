//! Hypothesis layer - groups hypotheses for the Paperproof view.

use std::collections::HashSet;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::{
    lean_rpc::Hypothesis,
    tui::widgets::{
        diff_text::{diff_style, DiffState, TaggedTextExt},
        ClickRegion, SelectableItem,
    },
};

/// A layer of hypotheses.
#[derive(Debug, Clone)]
pub struct HypLayer {
    pub hypotheses: Vec<(usize, usize, Hypothesis)>, // (goal_idx, hyp_idx, hyp)
}

impl HypLayer {
    pub const fn new() -> Self {
        Self {
            hypotheses: Vec::new(),
        }
    }

    pub fn add(&mut self, goal_idx: usize, hyp_idx: usize, hyp: Hypothesis) {
        self.hypotheses.push((goal_idx, hyp_idx, hyp));
    }

    pub const fn len(&self) -> usize {
        self.hypotheses.len()
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn render(
        &self,
        selected: Option<SelectableItem>,
        base_y: u16,
        area: Rect,
        click_regions: &mut Vec<ClickRegion>,
        depends_on: &HashSet<String>,
    ) -> Vec<Line<'static>> {
        self.hypotheses
            .iter()
            .enumerate()
            .map(|(i, (goal_idx, hyp_idx, hyp))| {
                let is_selected = selected
                    == Some(SelectableItem::Hypothesis {
                        goal_idx: *goal_idx,
                        hyp_idx: *hyp_idx,
                    });
                let is_dependency = hyp.names.iter().any(|n| depends_on.contains(n));

                let y = base_y + i as u16;
                if y < area.y + area.height {
                    click_regions.push(ClickRegion {
                        area: Rect::new(area.x, y, area.width, 1),
                        item: SelectableItem::Hypothesis {
                            goal_idx: *goal_idx,
                            hyp_idx: *hyp_idx,
                        },
                    });
                }

                render_hyp_line(hyp, is_selected, is_dependency)
            })
            .collect()
    }
}

fn render_hyp_line(hyp: &Hypothesis, is_selected: bool, is_dependency: bool) -> Line<'static> {
    let state = DiffState {
        is_inserted: hyp.is_inserted,
        is_removed: hyp.is_removed,
        has_diff: hyp.type_.has_any_diff(),
    };
    let diff = diff_style(&state, is_selected, Color::White);

    let marker = match (
        is_dependency,
        hyp.is_inserted,
        hyp.is_removed,
        hyp.type_.has_any_diff(),
    ) {
        (true, _, _, _) => Span::styled(
            "[*]",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        (_, true, _, _) => Span::styled("[+]", Style::new().fg(Color::Green)),
        (_, _, true, _) => Span::styled("[-]", Style::new().fg(Color::Red)),
        (_, _, _, true) => Span::styled("[~]", Style::new().fg(Color::Yellow)),
        _ => Span::styled("   ", Style::new().fg(Color::DarkGray)),
    };

    let selection = if is_selected { "▶ " } else { "  " };
    let names = hyp.names.join(", ");

    // Apply bold styling to dependency hypotheses
    let name_style = if is_dependency {
        diff.style
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        diff.style
    };

    let mut spans = vec![
        marker,
        Span::styled(selection.to_string(), diff.style),
        Span::styled(format!("{names} : "), name_style),
    ];
    spans.extend(hyp.type_.to_spans(diff.style));
    Line::from(spans)
}
