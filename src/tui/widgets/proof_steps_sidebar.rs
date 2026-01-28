//! Proof steps sidebar widget for the Paperproof view.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget,
    },
};

use crate::{
    tui::widgets::{theme::Theme, InteractiveStatefulWidget},
    tui_ipc::{ProofDag, ProofDagNode},
};

// Tree drawing characters
const BRANCH_MID: &str = "├─ "; // Branch to non-last child
const BRANCH_END: &str = "╰─ "; // Branch to last child (rounded)
const VERT_LINE: &str = "│  "; // Vertical continuation
const EMPTY: &str = "   "; // Empty space after last child

/// State for the proof steps sidebar widget.
#[derive(Default)]
pub struct ProofStepsSidebarState {
    proof_dag: Option<ProofDag>,
    scroll_state: ScrollbarState,
    vertical_scroll: usize,
    horizontal_scroll: usize,
    /// Whether this pane is currently focused.
    is_focused: bool,
    /// Whether the user has taken manual control of scrolling.
    manual_scroll: bool,
    /// Previous current node ID, used to detect changes.
    prev_current_node: Option<u32>,
    /// Max content width (cached from last render).
    max_content_width: usize,
    /// Viewport width (cached from last render).
    viewport_width: usize,
}

impl ProofStepsSidebarState {
    fn build_lines(&self) -> Vec<Line<'static>> {
        let Some(dag) = &self.proof_dag else {
            return vec![];
        };

        // Track which depth levels are still "active" (have more siblings to come)
        let mut active_levels: Vec<bool> = Vec::new();
        let mut lines = Vec::new();

        for node in dag.dfs_iter() {
            let is_current = dag.is_current(node.id);

            // Determine if this node is the last child of its parent
            let is_last_child = node
                .parent
                .and_then(|pid| dag.get(pid))
                .is_none_or(|parent| parent.children.last() == Some(&node.id));

            // Adjust active_levels to match current depth
            update_active_levels(&mut active_levels, node.depth, is_last_child);

            // Build the tree prefix
            let prefix = build_tree_prefix(&active_levels, node.depth, is_last_child);
            lines.push(step_line(node, &prefix, is_current));

            // For continuation lines, use the same prefix structure but with vertical lines
            let cont_prefix = build_continuation_prefix(&active_levels, node.depth, is_last_child);

            if let Some(deps) = dependency_line(node, &cont_prefix) {
                lines.push(deps);
            }
            if let Some(thms) = theorem_line(node, &cont_prefix) {
                lines.push(thms);
            }
        }

        lines
    }

    /// Calculate scroll position to center the current step in the viewport.
    fn calculate_centered_scroll(&self, viewport_height: usize) -> usize {
        let Some(dag) = &self.proof_dag else {
            return 0;
        };

        let mut current_line: usize = 0;
        for node in dag.dfs_iter() {
            if dag.is_current(node.id) {
                break;
            }
            current_line += 1; // Main step line
            if !node.tactic.depends_on.is_empty() {
                current_line += 1; // Dependency line
            }
            if !node.tactic.theorems_used.is_empty() {
                current_line += 1; // Theorem line
            }
        }

        // Center the current line in the viewport
        let half_viewport = viewport_height / 2;
        let total = self.total_lines();
        let max_scroll = total.saturating_sub(viewport_height);

        current_line.saturating_sub(half_viewport).min(max_scroll)
    }

    /// Total number of lines in the proof steps view.
    fn total_lines(&self) -> usize {
        let Some(dag) = &self.proof_dag else {
            return 0;
        };

        let mut count = 0;
        for node in dag.dfs_iter() {
            count += 1;
            if !node.tactic.depends_on.is_empty() {
                count += 1;
            }
            if !node.tactic.theorems_used.is_empty() {
                count += 1;
            }
        }
        count
    }

    /// Set focus state.
    pub const fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;
    }

    /// Scroll up by one line.
    pub const fn scroll_up(&mut self) {
        self.manual_scroll = true;
        self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
    }

    /// Scroll down by one line.
    pub fn scroll_down(&mut self, viewport_height: usize) {
        self.manual_scroll = true;
        let max_scroll = self.total_lines().saturating_sub(viewport_height);
        if self.vertical_scroll < max_scroll {
            self.vertical_scroll += 1;
        }
    }

    /// Scroll left by one column.
    pub const fn scroll_left(&mut self) {
        self.manual_scroll = true;
        self.horizontal_scroll = self.horizontal_scroll.saturating_sub(1);
    }

    /// Scroll right by one column (bounded by content width).
    pub const fn scroll_right(&mut self) {
        self.manual_scroll = true;
        let max_horiz = self.max_content_width.saturating_sub(self.viewport_width);
        if self.horizontal_scroll < max_horiz {
            self.horizontal_scroll += 1;
        }
    }

    /// Reset to auto-scroll mode (following current step).
    pub const fn reset_scroll(&mut self) {
        self.manual_scroll = false;
    }
}

/// Update the `active_levels` vector for the current node.
fn update_active_levels(active_levels: &mut Vec<bool>, depth: usize, is_last_child: bool) {
    active_levels.truncate(depth);
    if depth > 0 {
        // Ensure we have entries for all ancestor levels
        active_levels.resize(depth, true);
        // The parent's level continues if this is not the last child
        if let Some(last) = active_levels.last_mut() {
            *last = !is_last_child;
        }
    }
}

/// Build the tree prefix for a node's main line.
fn build_tree_prefix(active_levels: &[bool], depth: usize, is_last_child: bool) -> String {
    if depth == 0 {
        return String::new();
    }

    let mut prefix = String::new();

    // For each ancestor level (except the last one), draw vertical line or empty
    for &active in active_levels.iter().take(depth.saturating_sub(1)) {
        prefix.push_str(if active { VERT_LINE } else { EMPTY });
    }

    // For the current level, draw branch
    prefix.push_str(if is_last_child {
        BRANCH_END
    } else {
        BRANCH_MID
    });

    prefix
}

/// Build the prefix for continuation lines (uses, thms).
fn build_continuation_prefix(active_levels: &[bool], depth: usize, is_last_child: bool) -> String {
    if depth == 0 {
        return String::from("   "); // Indent for root node's continuation
    }

    let mut prefix = String::new();

    // For each ancestor level, draw vertical line or empty
    for &active in active_levels.iter().take(depth.saturating_sub(1)) {
        prefix.push_str(if active { VERT_LINE } else { EMPTY });
    }

    // After the branch point, use vertical line if not last, empty if last
    prefix.push_str(if is_last_child { EMPTY } else { VERT_LINE });

    prefix
}

/// Widget displaying proof steps in a sidebar.
pub struct ProofStepsSidebar;

impl InteractiveStatefulWidget for ProofStepsSidebar {
    type Input = Option<ProofDag>;
    type Event = ();

    fn update_state(state: &mut Self::State, input: Self::Input) {
        let new_current = input.as_ref().and_then(|dag| dag.current_node);

        // Reset manual scroll when current node changes (re-center on new step)
        if new_current != state.prev_current_node {
            state.manual_scroll = false;
            state.prev_current_node = new_current;
        }

        state.proof_dag = input;
    }
}

impl StatefulWidget for ProofStepsSidebar {
    type State = ProofStepsSidebarState;

    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let border_style = if state.is_focused {
            Style::new().fg(Theme::BORDER_FOCUSED)
        } else {
            Theme::DIM
        };
        let title = if state.is_focused {
            "▶ Proof Steps "
        } else {
            " Proof Steps "
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title)
            .title_style(Style::new().fg(Color::Yellow));

        let inner = block.inner(area);
        block.render(area, buf);

        let lines = state.build_lines();
        let total_lines = lines.len();
        let viewport_height = inner.height as usize;
        let viewport_width = inner.width as usize;

        // Cache dimensions for scroll bounds
        state.viewport_width = viewport_width;
        state.max_content_width = lines.iter().map(Line::width).max().unwrap_or(0);

        // Auto-scroll to center current step unless user has taken manual control
        if !state.manual_scroll {
            state.vertical_scroll = state.calculate_centered_scroll(viewport_height);
        }

        // Clamp scroll values to valid range
        let max_vert_scroll = total_lines.saturating_sub(viewport_height);
        let max_horiz_scroll = state.max_content_width.saturating_sub(viewport_width);
        state.vertical_scroll = state.vertical_scroll.min(max_vert_scroll);
        state.horizontal_scroll = state.horizontal_scroll.min(max_horiz_scroll);

        // Update scrollbar state
        state.scroll_state = state
            .scroll_state
            .content_length(total_lines)
            .position(state.vertical_scroll);

        Paragraph::new(lines)
            .scroll((state.vertical_scroll as u16, state.horizontal_scroll as u16))
            .render(inner, buf);

        // Render scrollbar if content overflows
        if total_lines > viewport_height && inner.width > 1 {
            let scrollbar_area = Rect {
                x: area.x + area.width.saturating_sub(1),
                y: area.y + 1, // Skip top border
                width: 1,
                height: area.height.saturating_sub(2), // Skip top and bottom borders
            };
            Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
                scrollbar_area,
                buf,
                &mut state.scroll_state,
            );
        }
    }
}

fn step_line(node: &ProofDagNode, prefix: &str, is_current: bool) -> Line<'static> {
    let style = if is_current {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::White)
    };

    let marker = if is_current { "▶ " } else { "  " };

    Line::from(vec![
        Span::styled(marker, Style::new().fg(Color::Cyan)),
        Span::styled(prefix.to_string(), Theme::DIM),
        Span::styled(node.tactic.text.clone(), style),
    ])
}

fn dependency_line(node: &ProofDagNode, prefix: &str) -> Option<Line<'static>> {
    if node.tactic.depends_on.is_empty() {
        return None;
    }

    Some(Line::from(vec![
        Span::raw("  "), // Match marker width
        Span::styled(prefix.to_string(), Theme::DIM),
        Span::styled(
            format!("uses: {}", node.tactic.depends_on.join(", ")),
            Theme::DEPENDENCY,
        ),
    ]))
}

fn theorem_line(node: &ProofDagNode, prefix: &str) -> Option<Line<'static>> {
    if node.tactic.theorems_used.is_empty() {
        return None;
    }

    Some(Line::from(vec![
        Span::raw("  "), // Match marker width
        Span::styled(prefix.to_string(), Theme::DIM),
        Span::styled(
            format!("thms: {}", node.tactic.theorems_used.join(", ")),
            Style::new().fg(Color::Blue).add_modifier(Modifier::DIM),
        ),
    ]))
}
