//! Proof steps sidebar component for the Paperproof view.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::{
    lean_rpc::PaperproofStep,
    tui_ipc::{ProofStep, ProofStepSource},
};

/// Input for rendering the proof steps sidebar.
pub struct ProofStepsSidebarInput<'a> {
    pub proof_steps: &'a [ProofStep],
    pub paperproof_steps: Option<&'a [PaperproofStep]>,
    pub current_step_index: usize,
}

/// Render the proof steps sidebar.
#[allow(clippy::cast_possible_truncation)]
pub fn render_proof_steps_sidebar(frame: &mut Frame, area: Rect, input: &ProofStepsSidebarInput<'_>) {
    let source = input.proof_steps.first().map_or("Local", |s| match s.source {
        ProofStepSource::Paperproof => "Paperproof",
        ProofStepSource::Local => "Local",
    });

    let mut lines = vec![
        Line::from(vec![
            Span::styled(format!("{source} "), Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(format!("({} steps)", input.proof_steps.len()), Style::new().fg(Color::DarkGray)),
        ]),
        Line::from(""),
    ];

    for (i, step) in input.proof_steps.iter().enumerate() {
        let is_current = i == input.current_step_index;
        render_step_line(&mut lines, step, i, is_current);
        render_step_dependencies(&mut lines, step);
        render_step_theorems(&mut lines, input.paperproof_steps, i);
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_step_line(lines: &mut Vec<Line<'static>>, step: &ProofStep, index: usize, is_current: bool) {
    let style = if is_current {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::White)
    };

    lines.push(Line::from(vec![
        Span::styled(format!("{:>3}.", index + 1), Style::new().fg(Color::DarkGray)),
        Span::styled(if is_current { "▶ " } else { "  " }, Style::new().fg(Color::Cyan)),
        Span::styled("  ".repeat(step.depth), Style::new().fg(Color::DarkGray)),
        Span::styled(step.tactic.clone(), style),
    ]));
}

fn render_step_dependencies(lines: &mut Vec<Line<'static>>, step: &ProofStep) {
    if !step.depends_on.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("     "),
            Span::styled(
                format!("uses: {}", step.depends_on.join(", ")),
                Style::new().fg(Color::Yellow).add_modifier(Modifier::DIM),
            ),
        ]));
    }
}

fn render_step_theorems(lines: &mut Vec<Line<'static>>, paperproof_steps: Option<&[PaperproofStep]>, index: usize) {
    let Some(pp_steps) = paperproof_steps else { return };
    let Some(pp_step) = pp_steps.get(index) else { return };
    if pp_step.theorems.is_empty() { return; }

    let thm_names: Vec<_> = pp_step.theorems.iter()
        .map(|t| t.name.as_str())
        .collect();

    lines.push(Line::from(vec![
        Span::raw("     "),
        Span::styled(
            format!("thms: {}", thm_names.join(", ")),
            Style::new().fg(Color::Blue).add_modifier(Modifier::DIM),
        ),
    ]));
}
