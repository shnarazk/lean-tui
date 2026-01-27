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

use super::Theme;
use crate::{lean_rpc::PaperproofStep, tui_ipc::ProofStep};

/// State for the proof steps sidebar widget.
#[derive(Default)]
pub struct ProofStepsSidebarState {
    proof_steps: Vec<ProofStep>,
    paperproof_steps: Option<Vec<PaperproofStep>>,
    current_step_index: usize,
    scroll_state: ScrollbarState,
    vertical_scroll: usize,
}

impl ProofStepsSidebarState {
    /// Update the state with new data.
    pub fn update(
        &mut self,
        proof_steps: Vec<ProofStep>,
        paperproof_steps: Option<Vec<PaperproofStep>>,
        current_step_index: usize,
    ) {
        self.proof_steps = proof_steps;
        self.paperproof_steps = paperproof_steps;
        self.current_step_index = current_step_index;
    }

    fn build_lines(&self) -> Vec<Line<'static>> {
        self.proof_steps
            .iter()
            .enumerate()
            .flat_map(|(i, step)| {
                let is_current = i == self.current_step_index;
                let mut lines = vec![step_line(step, i, is_current)];

                if let Some(deps) = dependency_line(step) {
                    lines.push(deps);
                }
                if let Some(thms) = theorem_line(self.paperproof_steps.as_deref(), i) {
                    lines.push(thms);
                }

                lines
            })
            .collect()
    }
}

/// Widget displaying proof steps in a sidebar.
pub struct ProofStepsSidebar;

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
        // Find the line index for the current step
        let mut line_count = 0;
        for (i, step) in state.proof_steps.iter().enumerate() {
            if i == state.current_step_index {
                state.vertical_scroll = line_count;
                break;
            }
            line_count += 1; // Main step line
            if !step.depends_on.is_empty() {
                line_count += 1; // Dependency line
            }
            let has_theorems = state
                .paperproof_steps
                .as_ref()
                .and_then(|pp_steps| pp_steps.get(i))
                .is_some_and(|pp_step| !pp_step.theorems.is_empty());
            if has_theorems {
                line_count += 1; // Theorem line
            }
        }

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

fn step_line(step: &ProofStep, index: usize, is_current: bool) -> Line<'static> {
    let style = if is_current {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::White)
    };

    let marker = if is_current { "▶ " } else { "  " };
    let indent = "  ".repeat(step.depth);

    Line::from(vec![
        Span::styled(format!("{:>3}.", index + 1), Theme::DIM),
        Span::styled(marker, Style::new().fg(Color::Cyan)),
        Span::styled(indent, Theme::DIM),
        Span::styled(step.tactic.clone(), style),
    ])
}

fn dependency_line(step: &ProofStep) -> Option<Line<'static>> {
    if step.depends_on.is_empty() {
        return None;
    }

    Some(Line::from(vec![
        Span::raw("     "),
        Span::styled(
            format!("uses: {}", step.depends_on.join(", ")),
            Theme::DEPENDENCY,
        ),
    ]))
}

fn theorem_line(
    paperproof_steps: Option<&[PaperproofStep]>,
    index: usize,
) -> Option<Line<'static>> {
    let pp_step = paperproof_steps?.get(index)?;

    if pp_step.theorems.is_empty() {
        return None;
    }

    let thm_names: String = pp_step
        .theorems
        .iter()
        .map(|t| t.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    Some(Line::from(vec![
        Span::raw("     "),
        Span::styled(
            format!("thms: {thm_names}"),
            Style::new().fg(Color::Blue).add_modifier(Modifier::DIM),
        ),
    ]))
}
