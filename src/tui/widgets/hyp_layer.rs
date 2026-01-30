//! Hypothesis layer - groups hypotheses for the tactic tree view.

use std::collections::HashSet;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::{
    lean_rpc::{Hypothesis, HypothesisInfo},
    tui::widgets::{
        diff_text::{diff_style, DiffState, TaggedTextExt},
        ClickRegion, Selection,
    },
};

/// Context for rendering a hypothesis layer.
pub struct HypLayerRenderContext<'a> {
    pub selected: Option<Selection>,
    pub base_y: u16,
    pub area: Rect,
    pub depends_on: &'a HashSet<String>,
}

/// A layer of hypotheses.
#[derive(Debug, Clone)]
pub struct HypLayer {
    /// Node ID for creating selections (from DAG).
    node_id: Option<u32>,
    pub hypotheses: Vec<(usize, Hypothesis)>, // (hyp_idx, hyp)
}

impl HypLayer {
    pub const fn new() -> Self {
        Self {
            node_id: None,
            hypotheses: Vec::new(),
        }
    }

    pub const fn set_node_id(&mut self, node_id: Option<u32>) {
        self.node_id = node_id;
    }

    /// Add a hypothesis from HypothesisInfo (used when working directly with
    /// ProofState).
    pub fn add_from_info(&mut self, hyp_idx: usize, info: &HypothesisInfo) {
        let hyp = Hypothesis {
            names: vec![info.name.clone()],
            type_: info.type_.clone(),
            val: info.value.clone(),
            is_instance: info.is_instance,
            is_type: false,
            fvar_ids: None,
            is_inserted: false,
            is_removed: info.is_removed,
            goto_locations: info.goto_locations.clone(),
        };
        self.hypotheses.push((hyp_idx, hyp));
    }

    pub const fn len(&self) -> usize {
        self.hypotheses.len()
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn render(
        &self,
        ctx: &HypLayerRenderContext<'_>,
        click_regions: &mut Vec<ClickRegion>,
    ) -> Vec<Line<'static>> {
        // Track click regions
        if let Some(node_id) = self.node_id {
            self.track_click_regions(click_regions, node_id, ctx);
        }

        self.hypotheses
            .iter()
            .map(|(hyp_idx, hyp)| {
                let is_selected = matches!(
                    ctx.selected,
                    Some(Selection::Hyp { hyp_idx: hi, .. }) if hi == *hyp_idx
                );
                let is_dependency = hyp.names.iter().any(|n| ctx.depends_on.contains(n));
                render_hyp_line(hyp, is_selected, is_dependency)
            })
            .collect()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn track_click_regions(
        &self,
        click_regions: &mut Vec<ClickRegion>,
        node_id: u32,
        ctx: &HypLayerRenderContext<'_>,
    ) {
        for (i, (hyp_idx, _)) in self.hypotheses.iter().enumerate() {
            let y = ctx.base_y + i as u16;
            if y >= ctx.area.y + ctx.area.height {
                break;
            }
            click_regions.push(ClickRegion {
                area: Rect::new(ctx.area.x, y, ctx.area.width, 1),
                selection: Selection::Hyp {
                    node_id,
                    hyp_idx: *hyp_idx,
                },
            });
        }
    }
}

const DIM_GRAY: Style = Style::new().fg(Color::DarkGray);

fn render_hyp_line(hyp: &Hypothesis, is_selected: bool, is_dependency: bool) -> Line<'static> {
    let state = DiffState {
        is_inserted: hyp.is_inserted,
        is_removed: hyp.is_removed,
        has_diff: hyp.type_.has_any_diff(),
    };
    let diff = diff_style(&state, is_selected, Color::White);

    // Simple dimmed markers like before_after mode
    let marker = match (
        is_dependency,
        hyp.is_inserted,
        hyp.is_removed,
        hyp.type_.has_any_diff(),
    ) {
        (true, _, _, _) => Span::styled("*", DIM_GRAY),
        (_, true, _, _) => Span::styled("+", DIM_GRAY),
        (_, _, true, _) => Span::styled("-", DIM_GRAY),
        (_, _, _, true) => Span::styled("~", DIM_GRAY),
        _ => Span::styled(" ", DIM_GRAY),
    };

    let names = hyp.names.join(", ");

    // Only underline when selected (not for dependencies)
    // Dependencies get bold name only
    let name_style = if is_dependency {
        diff.style.add_modifier(Modifier::BOLD)
    } else {
        diff.style
    };

    let mut spans = vec![
        marker,
        Span::raw(" "),
        Span::styled(format!("{names} : "), name_style),
    ];
    // Type spans use diff.style which applies underline only when selected
    spans.extend(hyp.type_.to_spans(diff.style));
    Line::from(spans)
}
