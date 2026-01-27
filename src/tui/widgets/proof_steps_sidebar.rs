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

        dag.dfs_iter()
            .enumerate()
            .flat_map(|(i, node)| {
                let mut lines = vec![step_line(node, i)];

                if let Some(deps) = dependency_line(node) {
                    lines.push(deps);
                }
                if let Some(thms) = theorem_line(node) {
                    lines.push(thms);
                }

                lines
            })
            .collect()
    }

    fn calculate_scroll_position(&self) -> usize {
        let Some(dag) = &self.proof_dag else {
            return 0;
        };

        let mut line_count = 0;
        for node in dag.dfs_iter() {
            if node.is_current {
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

fn step_line(node: &ProofDagNode, index: usize) -> Line<'static> {
    let style = if node.is_current {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::White)
    };

    let marker = if node.is_current { "▶ " } else { "  " };
    let indent = "  ".repeat(node.depth);

    Line::from(vec![
        Span::styled(format!("{:>3}.", index + 1), Theme::DIM),
        Span::styled(marker, Style::new().fg(Color::Cyan)),
        Span::styled(indent, Theme::DIM),
        Span::styled(node.tactic.text.clone(), style),
    ])
}

fn dependency_line(node: &ProofDagNode) -> Option<Line<'static>> {
    if node.tactic.depends_on.is_empty() {
        return None;
    }

    Some(Line::from(vec![
        Span::raw("     "),
        Span::styled(
            format!("uses: {}", node.tactic.depends_on.join(", ")),
            Theme::DEPENDENCY,
        ),
    ]))
}

fn theorem_line(node: &ProofDagNode) -> Option<Line<'static>> {
    if node.tactic.theorems_used.is_empty() {
        return None;
    }

    Some(Line::from(vec![
        Span::raw("     "),
        Span::styled(
            format!("thms: {}", node.tactic.theorems_used.join(", ")),
            Style::new().fg(Color::Blue).add_modifier(Modifier::DIM),
        ),
    ]))
}
