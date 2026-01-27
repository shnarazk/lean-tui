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
    tui::components::{ClickRegion, Component, HypothesisFilters, SelectableItem},
};

/// Input for updating the hypothesis section.
pub struct HypSectionInput {
    pub goals: Vec<Goal>,
    pub filters: HypothesisFilters,
    pub depends_on: HashSet<String>,
    pub selection: Option<SelectableItem>,
}

#[derive(Default)]
pub struct HypSection {
    layer: HypLayer,
    depends_on: HashSet<String>,
    selection: Option<SelectableItem>,
    click_regions: Vec<ClickRegion>,
}

impl Default for HypLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl HypSection {
    /// Get the click regions computed during the last render.
    pub fn click_regions(&self) -> &[ClickRegion] {
        &self.click_regions
    }
}

impl Component for HypSection {
    type Input = HypSectionInput;
    type Event = ();

    fn update(&mut self, input: Self::Input) {
        self.layer = HypLayer::new();
        self.depends_on = input.depends_on;
        self.selection = input.selection;
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
    fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.click_regions.clear();

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

        let lines = self.layer.render(self.selection, inner.y, inner, &mut self.click_regions, &self.depends_on);
        frame.render_widget(Paragraph::new(lines), inner);
    }
}
