//! Given pane - displays initial hypotheses in the semantic tableau.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};

use super::{ClickRegion, Selection};
use crate::tui::widgets::theme::Theme;
use crate::tui_ipc::HypothesisInfo;

/// State for the given pane.
#[derive(Default)]
pub struct GivenPaneState {
    /// Click regions from last render.
    pub click_regions: Vec<ClickRegion>,
}

impl GivenPaneState {
    /// Find click at position.
    pub fn find_click_at(&self, x: u16, y: u16) -> Option<Selection> {
        self.click_regions
            .iter()
            .find(|r| {
                x >= r.area.x
                    && x < r.area.x + r.area.width
                    && y >= r.area.y
                    && y < r.area.y + r.area.height
            })
            .map(|r| r.selection)
    }
}

/// Given pane widget - displays initial hypotheses.
pub struct GivenPane<'a> {
    hypotheses: &'a [HypothesisInfo],
    selection: Option<Selection>,
}

impl<'a> GivenPane<'a> {
    pub const fn new(hypotheses: &'a [HypothesisInfo], selection: Option<Selection>) -> Self {
        Self {
            hypotheses,
            selection,
        }
    }
}

impl StatefulWidget for GivenPane<'_> {
    type State = GivenPaneState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(Theme::PROOF_HYP_FG))
            .title(Span::styled(
                " Given ",
                Style::new()
                    .fg(Theme::PROOF_HYP_FG)
                    .add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 2 || inner.height < 1 {
            return;
        }

        let visible_hyps: Vec<_> = self
            .hypotheses
            .iter()
            .enumerate()
            .filter(|(_, h)| !h.is_proof)
            .collect();

        if visible_hyps.is_empty() {
            Paragraph::new("(no hypotheses)")
                .style(Style::new().fg(Color::DarkGray))
                .render(inner, buf);
            return;
        }

        let mut x_offset = inner.x;
        let hyp_spans: Vec<Span> = visible_hyps
            .iter()
            .take(5)
            .enumerate()
            .flat_map(|(i, (hyp_idx, h))| {
                let mut spans = vec![];
                if i > 0 {
                    spans.push(Span::raw(" "));
                    x_offset += 1;
                }

                let is_selected = matches!(
                    self.selection,
                    Some(Selection::InitialHyp { hyp_idx: hi }) if hi == *hyp_idx
                );

                let (fg, bg) = hyp_style_colors(h.is_proof);
                let mut style = Style::new().fg(fg).bg(bg);
                if is_selected {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }

                let truncated_type = truncate_str(&h.type_, 20);
                let text = format!(" {}: {} ", h.name, truncated_type);
                let text_width = text.chars().count() as u16;

                // Track click region
                state.click_regions.push(ClickRegion {
                    area: Rect::new(x_offset, inner.y, text_width.min(inner.width), 1),
                    selection: Selection::InitialHyp { hyp_idx: *hyp_idx },
                });
                x_offset += text_width;

                spans.push(Span::styled(text, style));
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

        Paragraph::new(line).render(inner, buf);
    }
}

/// Truncate a string to max length with ellipsis.
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

/// Get hypothesis style colors based on proof status.
pub const fn hyp_style_colors(is_proof: bool) -> (Color, Color) {
    if is_proof {
        (Theme::PROOF_HYP_FG, Theme::PROOF_HYP_BG)
    } else {
        (Theme::DATA_HYP_FG, Theme::DATA_HYP_BG)
    }
}
