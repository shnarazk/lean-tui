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
const BRANCH_MID: &str = "├─ ";   // Branch to non-last child
const BRANCH_END: &str = "╰─ ";   // Branch to last child (rounded)
const VERT_LINE: &str = "│  ";    // Vertical continuation
const EMPTY: &str = "   ";        // Empty space after last child

/// State for the proof steps sidebar widget.
#[derive(Default)]
pub struct ProofStepsSidebarState {
    proof_dag: Option<ProofDag>,
    scroll_state: ScrollbarState,
    vertical_scroll: usize,
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

    fn calculate_scroll_position(&self) -> usize {
        let Some(dag) = &self.proof_dag else {
            return 0;
        };

        let mut line_count = 0;
        for node in dag.dfs_iter() {
            if dag.is_current(node.id) {
                return line_count;
            }
            line_count += 1; // Main step line
            if !node.tactic.depends_on.is_empty() {
                line_count += 1; // Dependency line
            }
            if !node.tactic.theorems_used.is_empty() {
                line_count += 1; // Theorem line
            }
        }
        0
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
    prefix.push_str(if is_last_child { BRANCH_END } else { BRANCH_MID });

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
        state.proof_dag = input;
    }
}

impl StatefulWidget for ProofStepsSidebar {
    type State = ProofStepsSidebarState;

    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::DIM)
            .title(" Proof Steps ")
            .title_style(Style::new().fg(Color::Yellow));

        let inner = block.inner(area);
        block.render(area, buf);

        let lines = state.build_lines();

        // Calculate scroll position based on current step
        state.vertical_scroll = state.calculate_scroll_position();

        let total_lines = lines.len();
        let viewport_height = inner.height as usize;

        // Update scrollbar state
        state.scroll_state = state
            .scroll_state
            .content_length(total_lines)
            .position(state.vertical_scroll);

        Paragraph::new(lines).render(inner, buf);

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
