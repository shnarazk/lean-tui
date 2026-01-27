//! Proof steps sidebar widget for the Paperproof view.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::{lean_rpc::PaperproofStep, tui_ipc::ProofStep};

/// Widget displaying proof steps in a sidebar.
pub struct ProofStepsSidebar<'a> {
    proof_steps: &'a [ProofStep],
    paperproof_steps: Option<&'a [PaperproofStep]>,
    current_step_index: usize,
}

impl<'a> ProofStepsSidebar<'a> {
    pub const fn new(
        proof_steps: &'a [ProofStep],
        paperproof_steps: Option<&'a [PaperproofStep]>,
        current_step_index: usize,
    ) -> Self {
        Self {
            proof_steps,
            paperproof_steps,
            current_step_index,
        }
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
                if let Some(thms) = theorem_line(self.paperproof_steps, i) {
                    lines.push(thms);
                }

                lines
            })
            .collect()
    }
}

impl Widget for ProofStepsSidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.build_lines();
        Paragraph::new(lines).render(area, buf);
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
        Span::styled(
            format!("{:>3}.", index + 1),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(marker, Style::new().fg(Color::Cyan)),
        Span::styled(indent, Style::new().fg(Color::DarkGray)),
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
            Style::new().fg(Color::Yellow).add_modifier(Modifier::DIM),
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
