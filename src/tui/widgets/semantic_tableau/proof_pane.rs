//! Proof pane - scrollable proof tree visualization widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};

use super::{
    canvas::VirtualCanvas,
    navigation::{build_navigation_regions, NavigationRegion},
    state_node::{StateNode, StateNodeState},
    tree_layout::{calculate_tree_layout, NodePosition, TreeLayout},
    ClickRegion, Selection,
};
use crate::lean_rpc::{ProofDag, ProofState};

/// State for the proof pane widget.
#[derive(Default)]
pub struct ProofPaneState {
    scroll_x: i32,
    scroll_y: i32,
    manual_scroll: bool,
    prev_current_node: Option<u32>,
    layout: TreeLayout,
    content_width: i32,
    content_height: i32,
    pub click_regions: Vec<ClickRegion>,
    pub navigation_regions: Vec<NavigationRegion>,
}

impl ProofPaneState {
    #[allow(dead_code)]
    pub fn scroll_up(&mut self) {
        self.manual_scroll = true;
        self.scroll_y = (self.scroll_y - 1).max(0);
    }

    #[allow(dead_code)]
    pub fn scroll_down(&mut self, viewport_height: u16) {
        self.manual_scroll = true;
        let max = (self.content_height - i32::from(viewport_height)).max(0);
        self.scroll_y = (self.scroll_y + 1).min(max);
    }

    #[allow(dead_code)]
    pub fn scroll_left(&mut self) {
        self.manual_scroll = true;
        self.scroll_x = (self.scroll_x - 1).max(0);
    }

    #[allow(dead_code)]
    pub fn scroll_right(&mut self, viewport_width: u16) {
        self.manual_scroll = true;
        let max = (self.content_width - i32::from(viewport_width)).max(0);
        self.scroll_x = (self.scroll_x + 1).min(max);
    }

    #[allow(dead_code)]
    pub const fn reset_scroll(&mut self) {
        self.manual_scroll = false;
    }

    pub fn update_current_node(&mut self, current_node: Option<u32>) {
        if current_node != self.prev_current_node {
            self.manual_scroll = false;
            self.prev_current_node = current_node;
        }
    }

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

    pub fn navigation_regions(&self) -> &[NavigationRegion] {
        &self.navigation_regions
    }
}

/// Proof pane widget.
pub struct ProofPane<'a> {
    dag: &'a ProofDag,
    top_down: bool,
    selection: Option<Selection>,
    /// Current proof state from LSP (used to override current node's state if
    /// different).
    current_state: &'a ProofState,
}

impl<'a> ProofPane<'a> {
    pub const fn new(
        dag: &'a ProofDag,
        top_down: bool,
        selection: Option<Selection>,
        current_state: &'a ProofState,
    ) -> Self {
        Self {
            dag,
            top_down,
            selection,
            current_state,
        }
    }
}

impl StatefulWidget for ProofPane<'_> {
    type State = ProofPaneState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();
        state.navigation_regions.clear();

        if self.dag.is_empty() || self.dag.root.is_none() {
            Paragraph::new("No proof steps")
                .style(Style::new().fg(Color::DarkGray))
                .render(area, buf);
            return;
        }

        // Calculate layout
        state.layout = calculate_tree_layout(self.dag, self.top_down);
        state.content_width = state.layout.content_width;
        state.content_height = state.layout.content_height;

        if state.layout.nodes.is_empty() {
            return;
        }

        state.navigation_regions = build_navigation_regions(self.dag, &state.layout);

        // Setup canvas
        let mut canvas = VirtualCanvas::new(state.content_width, state.content_height);

        if state.manual_scroll {
            let max_x = (state.content_width - i32::from(area.width)).max(0);
            let max_y = (state.content_height - i32::from(area.height)).max(0);
            state.scroll_x = state.scroll_x.clamp(0, max_x);
            state.scroll_y = state.scroll_y.clamp(0, max_y);
            canvas.scroll_x = state.scroll_x;
            canvas.scroll_y = state.scroll_y;
        } else if let Some(target) = find_scroll_target(&state.layout, self.dag, self.selection) {
            canvas.scroll_to_center(target.x, target.y, target.width, target.height, area);
            state.scroll_x = canvas.scroll_x;
            state.scroll_y = canvas.scroll_y;
        }

        // Render nodes using StateNode widget
        for pos in &state.layout.nodes {
            let Some(render_area) =
                canvas.clip_to_viewport(pos.x, pos.y, pos.width, pos.height, area)
            else {
                continue;
            };
            let Some(node) = self.dag.get(pos.node_id) else {
                continue;
            };
            if render_area.width < 3 || render_area.height < 3 {
                continue;
            }

            let is_current = self.dag.is_current(pos.node_id);
            // For the current node, use actual LSP state if it differs from node's state
            let override_state = if is_current && !self.current_state.goals.is_empty() {
                Some(self.current_state)
            } else {
                None
            };

            let node_widget = StateNode::new(
                node,
                is_current,
                self.selection,
                self.top_down,
                override_state,
            );
            let mut node_state = StateNodeState::default();
            node_widget.render(render_area, buf, &mut node_state);
            state.click_regions.extend(node_state.click_regions);
        }

        // Scrollbars
        render_scrollbars(buf, area, &canvas);
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn render_scrollbars(buf: &mut Buffer, area: Rect, canvas: &VirtualCanvas) {
    if canvas.needs_vertical_scroll(area) && area.height > 2 {
        let sb_area = Rect::new(
            area.x + area.width.saturating_sub(1),
            area.y,
            1,
            area.height,
        );
        let mut state = ScrollbarState::new(canvas.content_height.max(0) as usize)
            .position(canvas.scroll_y.max(0) as usize)
            .viewport_content_length(area.height as usize);
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .render(sb_area, buf, &mut state);
    }

    if canvas.needs_horizontal_scroll(area) && area.width > 2 {
        let sb_area = Rect::new(
            area.x,
            area.y + area.height.saturating_sub(1),
            area.width.saturating_sub(1),
            1,
        );
        let mut state = ScrollbarState::new(canvas.content_width.max(0) as usize)
            .position(canvas.scroll_x.max(0) as usize)
            .viewport_content_length(area.width as usize);
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .begin_symbol(Some("◀"))
            .end_symbol(Some("▶"))
            .render(sb_area, buf, &mut state);
    }
}

fn find_scroll_target<'a>(
    layout: &'a TreeLayout,
    dag: &ProofDag,
    selection: Option<Selection>,
) -> Option<&'a NodePosition> {
    if let Some(sel) = selection {
        let node_id = match sel {
            Selection::Goal { node_id, .. } | Selection::Hyp { node_id, .. } => Some(node_id),
            _ => None,
        };
        if let Some(nid) = node_id {
            if let Some(pos) = layout.find_node(nid) {
                return Some(pos);
            }
        }
    }
    dag.current_node
        .and_then(|id| layout.find_node(id))
        .or_else(|| dag.root.and_then(|id| layout.find_node(id)))
}
