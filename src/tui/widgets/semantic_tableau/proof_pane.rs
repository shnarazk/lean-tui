//! Proof pane - scrollable proof tree visualization widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget, Wrap},
};

use super::{
    canvas::VirtualCanvas,
    given_pane::{hyp_style_colors, truncate_str},
    navigation::{build_navigation_regions, NavigationRegion},
    tree_layout::{calculate_tree_layout, NodePosition, TreeLayout},
    ClickRegion, Selection,
};
use crate::tui::widgets::theme::Theme;
use crate::tui_ipc::{ProofDag, ProofDagNode};

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
        self.click_regions.iter().find(|r| {
            x >= r.area.x && x < r.area.x + r.area.width &&
            y >= r.area.y && y < r.area.y + r.area.height
        }).map(|r| r.selection)
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
}

impl<'a> ProofPane<'a> {
    pub const fn new(dag: &'a ProofDag, top_down: bool, selection: Option<Selection>) -> Self {
        Self { dag, top_down, selection }
    }
}

impl StatefulWidget for ProofPane<'_> {
    type State = ProofPaneState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();
        state.navigation_regions.clear();

        if self.dag.is_empty() || self.dag.root.is_none() {
            Paragraph::new("No proof steps").style(Style::new().fg(Color::DarkGray)).render(area, buf);
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

        // Render nodes
        for pos in &state.layout.nodes {
            let Some(render_area) = canvas.clip_to_viewport(pos.x, pos.y, pos.width, pos.height, area) else {
                continue;
            };
            let Some(node) = self.dag.get(pos.node_id) else { continue };
            if render_area.width < 3 || render_area.height < 3 { continue; }

            render_node(buf, render_area, node, self.dag.is_current(pos.node_id), self.selection, self.top_down, &mut state.click_regions);
        }

        // Scrollbars
        render_scrollbars(buf, area, &canvas);
    }
}

fn render_node(
    buf: &mut Buffer,
    area: Rect,
    node: &ProofDagNode,
    is_current: bool,
    selection: Option<Selection>,
    top_down: bool,
    click_regions: &mut Vec<ClickRegion>,
) {
    let border_color = node_border_color(node, is_current);
    let border_style = Style::new().fg(border_color).add_modifier(if is_current { Modifier::BOLD } else { Modifier::empty() });

    let title = if node.children.len() > 1 {
        format!(" {} [{}→] ", node.tactic.text, node.children.len())
    } else {
        format!(" {} ", node.tactic.text)
    };

    let title_style = Style::new()
        .fg(if is_current { Color::White } else { Color::Gray })
        .add_modifier(if is_current { Modifier::BOLD } else { Modifier::empty() });

    let arrow = if top_down { "▼" } else { "▲" };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(title, title_style))
        .title_bottom(Span::styled(format!(" {arrow} "), border_style));

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height < 1 { return; }

    // Build lines
    let hyps_line = build_hyps_line(node, selection);
    let goals_line = build_goals_line(node, selection);

    let lines: Vec<Line> = match (top_down, hyps_line) {
        (true, Some(h)) => vec![h, goals_line],
        (false, Some(h)) => vec![goals_line, h],
        (_, None) => vec![goals_line],
    };

    Paragraph::new(lines).wrap(Wrap { trim: true }).render(inner, buf);

    // Click regions
    let has_hyps = !node.new_hypotheses.is_empty();
    let (hyps_y, goals_y) = if top_down {
        (inner.y, if has_hyps { inner.y + 1 } else { inner.y })
    } else {
        (if has_hyps { inner.y + 1 } else { inner.y }, inner.y)
    };

    for (goal_idx, _) in node.state_after.goals.iter().enumerate() {
        click_regions.push(ClickRegion {
            area: Rect::new(inner.x, goals_y, inner.width, 1),
            selection: Selection::Goal { node_id: node.id, goal_idx },
        });
    }

    if has_hyps {
        for &hyp_idx in &node.new_hypotheses {
            click_regions.push(ClickRegion {
                area: Rect::new(inner.x, hyps_y, inner.width, 1),
                selection: Selection::Hyp { node_id: node.id, hyp_idx },
            });
        }
    }
}

const fn node_border_color(node: &ProofDagNode, is_current: bool) -> Color {
    if is_current {
        Theme::CURRENT_NODE_BORDER
    } else if node.is_leaf() && !node.is_complete() {
        Theme::INCOMPLETE_NODE_BORDER
    } else if node.is_leaf() && node.is_complete() {
        Theme::COMPLETED_NODE_BORDER
    } else {
        Theme::TACTIC_BORDER
    }
}

fn build_hyps_line(node: &ProofDagNode, selection: Option<Selection>) -> Option<Line<'static>> {
    if node.new_hypotheses.is_empty() { return None; }

    let mut spans: Vec<Span> = Vec::new();
    for (i, &hyp_idx) in node.new_hypotheses.iter().take(3).enumerate() {
        if i > 0 { spans.push(Span::raw(" ")); }
        if let Some(h) = node.state_after.hypotheses.get(hyp_idx) {
            let selected = matches!(selection, Some(Selection::Hyp { node_id, hyp_idx: hi }) if node_id == node.id && hi == hyp_idx);
            let (fg, bg) = hyp_style_colors(h.is_proof);
            let mut style = Style::new().fg(fg).bg(bg);
            if selected { style = style.add_modifier(Modifier::UNDERLINED); }
            spans.push(Span::styled(format!(" {}: {} ", h.name, truncate_str(&h.type_, 15)), style));
        }
    }
    if node.new_hypotheses.len() > 3 {
        spans.push(Span::styled(format!(" +{}", node.new_hypotheses.len() - 3), Style::new().fg(Color::DarkGray)));
    }
    Some(Line::from(spans))
}

fn build_goals_line(node: &ProofDagNode, selection: Option<Selection>) -> Line<'static> {
    if node.is_complete() {
        return Line::from(vec![Span::styled("✓ Goal completed", Style::new().fg(Theme::COMPLETED_GOAL_FG).add_modifier(Modifier::BOLD))]);
    }

    let mut spans: Vec<Span> = Vec::new();
    if node.is_leaf() {
        spans.push(Span::styled("⋯ ", Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    }

    for (goal_idx, g) in node.state_after.goals.iter().enumerate() {
        if goal_idx > 0 { spans.push(Span::styled(" │ ", Style::new().fg(Color::DarkGray))); }

        let selected = matches!(selection, Some(Selection::Goal { node_id, goal_idx: gi }) if node_id == node.id && gi == goal_idx);
        let underline = if selected { Modifier::UNDERLINED } else { Modifier::empty() };
        let goal_type = truncate_str(&g.type_, 35);

        if !g.username.is_empty() && g.username != "[anonymous]" {
            spans.push(Span::styled(format!("{}: ", g.username), Style::new().fg(Color::Cyan).add_modifier(underline)));
        }
        spans.push(Span::styled(format!("⊢ {goal_type}"), Style::new().fg(Theme::GOAL_FG).add_modifier(underline)));
    }

    if spans.is_empty() {
        spans.push(Span::styled("⊢ ...", Style::new().fg(Theme::GOAL_FG)));
    }
    Line::from(spans)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn render_scrollbars(buf: &mut Buffer, area: Rect, canvas: &VirtualCanvas) {
    if canvas.needs_vertical_scroll(area) && area.height > 2 {
        let sb_area = Rect::new(area.x + area.width.saturating_sub(1), area.y, 1, area.height);
        let mut state = ScrollbarState::new(canvas.content_height.max(0) as usize)
            .position(canvas.scroll_y.max(0) as usize)
            .viewport_content_length(area.height as usize);
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲")).end_symbol(Some("▼"))
            .render(sb_area, buf, &mut state);
    }

    if canvas.needs_horizontal_scroll(area) && area.width > 2 {
        let sb_area = Rect::new(area.x, area.y + area.height.saturating_sub(1), area.width.saturating_sub(1), 1);
        let mut state = ScrollbarState::new(canvas.content_width.max(0) as usize)
            .position(canvas.scroll_x.max(0) as usize)
            .viewport_content_length(area.width as usize);
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .begin_symbol(Some("◀")).end_symbol(Some("▶"))
            .render(sb_area, buf, &mut state);
    }
}

fn find_scroll_target<'a>(layout: &'a TreeLayout, dag: &ProofDag, selection: Option<Selection>) -> Option<&'a NodePosition> {
    if let Some(sel) = selection {
        let node_id = match sel {
            Selection::Goal { node_id, .. } | Selection::Hyp { node_id, .. } => Some(node_id),
            _ => None,
        };
        if let Some(nid) = node_id {
            if let Some(pos) = layout.find_node(nid) { return Some(pos); }
        }
    }
    dag.current_node.and_then(|id| layout.find_node(id))
        .or_else(|| dag.root.and_then(|id| layout.find_node(id)))
}
