//! Main Paperproof view component.

use std::iter;

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::{
    goal_section::GoalSection,
    hyp_section::{HypSection, HypSectionInput},
    tactic_row::render_divider,
};
use crate::{
    lean_rpc::Goal,
    tui::components::{
        hypothesis_indices, ClickRegion, Component, HypothesisFilters, KeyMouseEvent,
        SelectableItem,
    },
    tui_ipc::{DefinitionInfo, ProofStep, ProofStepSource},
};

/// Input for updating the Paperproof view.
pub struct PaperproofViewInput {
    pub goals: Vec<Goal>,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub proof_steps: Vec<ProofStep>,
    pub current_step_index: usize,
}

/// The main Paperproof view component.
#[derive(Default)]
pub struct PaperproofView {
    goals: Vec<Goal>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    proof_steps: Vec<ProofStep>,
    current_step_index: usize,
    hyp_section: HypSection,
    goal_section: GoalSection,
    filters: HypothesisFilters,
    selected_index: Option<usize>,
    click_regions: Vec<ClickRegion>,
}

impl PaperproofView {
    pub fn current_selection(&self) -> Option<SelectableItem> {
        let items = self.selectable_items();
        self.selected_index.and_then(|i| items.get(i).copied())
    }

    fn selectable_items(&self) -> Vec<SelectableItem> {
        self.goals
            .iter()
            .enumerate()
            .flat_map(|(goal_idx, goal)| {
                let hyps = hypothesis_indices(goal.hyps.len(), self.filters.reverse_order)
                    .filter(|&i| self.filters.should_show(&goal.hyps[i]))
                    .map(move |i| SelectableItem::Hypothesis { goal_idx, hyp_idx: i });
                hyps.chain(iter::once(SelectableItem::GoalTarget { goal_idx }))
            })
            .collect()
    }

    fn select_previous(&mut self) {
        let count = self.selectable_items().len();
        if count > 0 {
            self.selected_index = Some(self.selected_index.map_or(0, |i| i.saturating_sub(1)));
        }
    }

    fn select_next(&mut self) {
        let count = self.selectable_items().len();
        if count > 0 {
            self.selected_index = Some(match self.selected_index {
                Some(i) if i < count - 1 => i + 1,
                Some(i) => i,
                None => 0,
            });
        }
    }

    fn handle_click(&mut self, x: u16, y: u16) -> bool {
        let clicked = self.click_regions.iter().find(|r| {
            r.area.x <= x && x < r.area.x + r.area.width &&
            r.area.y <= y && y < r.area.y + r.area.height
        });
        if let Some(region) = clicked {
            let items = self.selectable_items();
            if let Some(idx) = items.iter().position(|i| *i == region.item) {
                self.selected_index = Some(idx);
                return true;
            }
        }
        false
    }

    const fn toggle_filter(&mut self, filter: FilterToggle) {
        match filter {
            FilterToggle::Instances => self.filters.hide_instances = !self.filters.hide_instances,
            FilterToggle::Types => self.filters.hide_types = !self.filters.hide_types,
            FilterToggle::Inaccessible => self.filters.hide_inaccessible = !self.filters.hide_inaccessible,
            FilterToggle::LetValues => self.filters.hide_let_values = !self.filters.hide_let_values,
            FilterToggle::ReverseOrder => self.filters.reverse_order = !self.filters.reverse_order,
            FilterToggle::Definition => self.filters.hide_definition = !self.filters.hide_definition,
        }
    }
}

#[derive(Clone, Copy)]
enum FilterToggle {
    Instances, Types, Inaccessible, LetValues, ReverseOrder, Definition,
}

impl Component for PaperproofView {
    type Input = PaperproofViewInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        let goals_changed = self.goals != input.goals;
        self.goals = input.goals;
        self.definition = input.definition;
        self.error = input.error;
        self.proof_steps = input.proof_steps;
        self.current_step_index = input.current_step_index;

        self.hyp_section.update(&HypSectionInput {
            goals: &self.goals,
            filters: self.filters,
        });

        if goals_changed {
            self.selected_index = (!self.selectable_items().is_empty()).then_some(0);
        }
    }

    fn handle_event(&mut self, event: Self::Event) -> bool {
        match event {
            KeyMouseEvent::Key(key) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => { self.select_next(); true }
                KeyCode::Char('k') | KeyCode::Up => { self.select_previous(); true }
                KeyCode::Char('i') => { self.toggle_filter(FilterToggle::Instances); true }
                KeyCode::Char('t') => { self.toggle_filter(FilterToggle::Types); true }
                KeyCode::Char('a') => { self.toggle_filter(FilterToggle::Inaccessible); true }
                KeyCode::Char('l') => { self.toggle_filter(FilterToggle::LetValues); true }
                KeyCode::Char('r') => { self.toggle_filter(FilterToggle::ReverseOrder); true }
                KeyCode::Char('d') => { self.toggle_filter(FilterToggle::Definition); true }
                _ => false,
            },
            KeyMouseEvent::Mouse(mouse) => {
                mouse.kind == MouseEventKind::Down(MouseButton::Left) && self.handle_click(mouse.column, mouse.row)
            }
        }
    }

    #[allow(clippy::option_if_let_else)]
    fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.click_regions.clear();

        let content = if let Some(ref error) = self.error {
            let [err, rest] = Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(area);
            frame.render_widget(Paragraph::new(format!("Error: {error}")).fg(Color::Red), err);
            rest
        } else {
            area
        };

        if self.goals.is_empty() {
            frame.render_widget(
                Paragraph::new("No goals").style(Style::new().fg(Color::DarkGray)),
                content,
            );
            return;
        }

        // Definition header
        let content = if let Some(def) = self.definition.as_ref().filter(|_| !self.filters.hide_definition) {
            let [hdr, rest] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(content);
            let header = Line::from(vec![
                Span::styled(&def.kind, Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(&def.name, Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]);
            frame.render_widget(Paragraph::new(header), hdr);
            rest
        } else {
            content
        };

        // Split: proof steps sidebar | main content
        let (main, steps_area) = if self.proof_steps.is_empty() {
            (content, None)
        } else {
            let [left, right] = Layout::horizontal([Constraint::Percentage(35), Constraint::Fill(1)]).areas(content);
            (right, Some(left))
        };

        if let Some(area) = steps_area {
            self.render_proof_steps(frame, area);
        }

        // Split main: hypotheses | divider | goals
        let [hyps, div, goals] = Layout::vertical([
            Constraint::Percentage(55), Constraint::Length(1), Constraint::Fill(1),
        ]).areas(main);

        self.hyp_section.render(frame, hyps, self.current_selection(), &mut self.click_regions);
        render_divider(frame, div, None);
        self.goal_section.render(frame, goals, &self.goals, self.current_selection(), &mut self.click_regions);
    }
}

impl PaperproofView {
    #[allow(clippy::cast_possible_truncation)]
    fn render_proof_steps(&self, frame: &mut Frame, area: Rect) {
        let source = self.proof_steps.first().map_or("Local", |s| match s.source {
            ProofStepSource::Paperproof => "Paperproof",
            ProofStepSource::Local => "Local",
        });

        let mut lines = vec![
            Line::from(vec![
                Span::styled(format!("{source} "), Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} steps)", self.proof_steps.len()), Style::new().fg(Color::DarkGray)),
            ]),
            Line::from(""),
        ];

        for (i, step) in self.proof_steps.iter().enumerate() {
            let is_current = i == self.current_step_index;
            let style = if is_current {
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::White)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{:>3}.", i + 1), Style::new().fg(Color::DarkGray)),
                Span::styled(if is_current { "▶ " } else { "  " }, Style::new().fg(Color::Cyan)),
                Span::styled("  ".repeat(step.depth), Style::new().fg(Color::DarkGray)),
                Span::styled(step.tactic.clone(), style),
            ]));

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

        frame.render_widget(Paragraph::new(lines), area);
    }
}
