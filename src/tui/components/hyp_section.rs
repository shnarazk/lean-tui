//! Hypotheses section - the top portion of the Paperproof view.

use std::collections::HashSet;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{
        Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget,
    },
};

use super::hyp_layer::HypLayer;
use crate::{
    lean_rpc::Goal,
    tui::components::{ClickRegion, HypothesisFilters, LayoutMetrics, SelectableItem, Theme},
};

impl Default for HypLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// State for the hypothesis section widget.
#[derive(Default)]
pub struct HypSectionState {
    layer: HypLayer,
    depends_on: HashSet<String>,
    selection: Option<SelectableItem>,
    click_regions: Vec<ClickRegion>,
    scroll_state: ScrollbarState,
    vertical_scroll: usize,
}

impl HypSectionState {
    /// Update the state with new data.
    pub fn update(
        &mut self,
        goals: &[Goal],
        filters: HypothesisFilters,
        depends_on: HashSet<String>,
        selection: Option<SelectableItem>,
    ) {
        self.layer = HypLayer::new();
        self.depends_on = depends_on;
        self.selection = selection;
        let mut seen: HashSet<String> = HashSet::new();

        let hyps = goals.iter().enumerate().flat_map(|(goal_idx, goal)| {
            goal.hyps
                .iter()
                .enumerate()
                .filter(|(_, hyp)| filters.should_show(hyp))
                .map(move |(hyp_idx, hyp)| (goal_idx, hyp_idx, hyp.clone()))
        });

        for (goal_idx, hyp_idx, hyp) in hyps {
            if seen.insert(hyp.names.join(",")) {
                self.layer.add(goal_idx, hyp_idx, hyp);
            }
        }
    }

    /// Get the click regions computed during the last render.
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

/// Widget for rendering the hypothesis section.
pub struct HypSection;

impl StatefulWidget for HypSection {
    type State = HypSectionState;

    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_style(Theme::DIM)
            .title(" Hypotheses ")
            .title_style(Style::new().fg(Theme::TITLE_HYPOTHESIS));

        let inner = block.inner(area);
        block.render(area, buf);

        if state.layer.len() == 0 {
            Paragraph::new("(no hypotheses)")
                .style(Theme::DIM.add_modifier(Modifier::ITALIC))
                .render(inner, buf);
            return;
        }

        // Calculate scroll position from selection
        if let Some(SelectableItem::Hypothesis { hyp_idx, .. }) = state.selection {
            state.vertical_scroll = LayoutMetrics::scroll_position(hyp_idx);
        }

        let total_hyps = state.layer.len();
        let viewport_height = inner.height as usize;

        // Update scrollbar state
        state.scroll_state = state
            .scroll_state
            .content_length(total_hyps)
            .position(state.vertical_scroll);

        let lines = state.layer.render(
            state.selection,
            inner.y,
            inner,
            &mut state.click_regions,
            &state.depends_on,
        );
        Paragraph::new(lines).render(inner, buf);

        // Render scrollbar if content overflows
        if total_hyps > viewport_height && inner.width > 1 {
            let scrollbar_area = Rect {
                x: area.x + area.width.saturating_sub(1),
                y: area.y + 1, // Skip top border
                width: 1,
                height: area.height.saturating_sub(1),
            };
            Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
                scrollbar_area,
                buf,
                &mut state.scroll_state,
            );
        }
    }
}
