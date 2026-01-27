//! Hypotheses section - the top portion of the Paperproof view.

use std::collections::HashSet;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::hyp_layer::HypLayer;
use crate::{
    lean_rpc::Goal,
    tui::components::{ClickRegion, HypothesisFilters, SelectableItem},
};

pub struct HypSectionInput<'a> {
    pub goals: &'a [Goal],
    pub filters: HypothesisFilters,
}

#[derive(Default)]
pub struct HypSection {
    layer: HypLayer,
}

impl Default for HypLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl HypSection {
    pub fn update(&mut self, input: &HypSectionInput<'_>) {
        self.layer = HypLayer::new();
        let mut seen: HashSet<String> = HashSet::new();

        let hyps = input.goals.iter().enumerate().flat_map(|(goal_idx, goal)| {
            goal.hyps.iter().enumerate()
                .filter(|(_, hyp)| input.filters.should_show(hyp))
                .map(move |(hyp_idx, hyp)| (goal_idx, hyp_idx, hyp.clone()))
        });

        for (goal_idx, hyp_idx, hyp) in hyps {
            if seen.insert(hyp.names.join(",")) {
                self.layer.add(goal_idx, hyp_idx, hyp);
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        selection: Option<SelectableItem>,
        click_regions: &mut Vec<ClickRegion>,
    ) {
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::new().fg(Color::DarkGray))
            .title(" Hypotheses ")
            .title_style(Style::new().fg(Color::Blue));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.layer.len() == 0 {
            frame.render_widget(
                Paragraph::new("(no hypotheses)")
                    .style(Style::new().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
                inner,
            );
            return;
        }

        let lines = self.layer.render(selection, inner.y, inner, click_regions);
        frame.render_widget(Paragraph::new(lines), inner);
    }
}
