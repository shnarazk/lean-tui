//! Steps mode - sidebar with proof steps plus hypotheses and goals sections.

use std::collections::HashSet;
use std::iter;

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::Stylize,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use crate::{
    lean_rpc::{Goal, PaperproofStep},
    tui::components::{
        hypothesis_indices, render_definition_header, render_divider, render_goal_before,
        render_proof_steps_sidebar, ClickRegion, Component, GoalSection, GoalSectionInput,
        HypSection, HypSectionInput, HypothesisFilters, KeyMouseEvent, ProofStepsSidebarInput,
        SelectableItem,
    },
    tui_ipc::{DefinitionInfo, ProofStep},
};

/// Input for updating the Steps mode.
pub struct StepsModeInput {
    pub goals: Vec<Goal>,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub proof_steps: Vec<ProofStep>,
    pub current_step_index: usize,
    pub paperproof_steps: Option<Vec<PaperproofStep>>,
}

/// Steps display mode - sidebar + hypotheses + goals.
#[derive(Default)]
pub struct StepsMode {
    goals: Vec<Goal>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    proof_steps: Vec<ProofStep>,
    current_step_index: usize,
    paperproof_steps: Option<Vec<PaperproofStep>>,
    hyp_section: HypSection,
    goal_section: GoalSection,
    filters: HypothesisFilters,
    selected_index: Option<usize>,
    click_regions: Vec<ClickRegion>,
    show_goal_before: bool,
}

impl StepsMode {
    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

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
            r.area.x <= x
                && x < r.area.x + r.area.width
                && r.area.y <= y
                && y < r.area.y + r.area.height
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
            FilterToggle::Inaccessible => {
                self.filters.hide_inaccessible = !self.filters.hide_inaccessible;
            }
            FilterToggle::LetValues => self.filters.hide_let_values = !self.filters.hide_let_values,
            FilterToggle::ReverseOrder => self.filters.reverse_order = !self.filters.reverse_order,
            FilterToggle::Definition => {
                self.filters.hide_definition = !self.filters.hide_definition;
            }
        }
    }
}

#[derive(Clone, Copy)]
enum FilterToggle {
    Instances,
    Types,
    Inaccessible,
    LetValues,
    ReverseOrder,
    Definition,
}

impl Component for StepsMode {
    type Input = StepsModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        let goals_changed = self.goals != input.goals;
        self.goals = input.goals;
        self.definition = input.definition;
        self.error = input.error;
        self.proof_steps = input.proof_steps;
        self.current_step_index = input.current_step_index;
        self.paperproof_steps = input.paperproof_steps;

        if goals_changed {
            self.selected_index = (!self.selectable_items().is_empty()).then_some(0);
        }
    }

    fn handle_event(&mut self, event: Self::Event) -> bool {
        match event {
            KeyMouseEvent::Key(key) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.select_next();
                    true
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.select_previous();
                    true
                }
                KeyCode::Char('i') => {
                    self.toggle_filter(FilterToggle::Instances);
                    true
                }
                KeyCode::Char('t') => {
                    self.toggle_filter(FilterToggle::Types);
                    true
                }
                KeyCode::Char('a') => {
                    self.toggle_filter(FilterToggle::Inaccessible);
                    true
                }
                KeyCode::Char('l') => {
                    self.toggle_filter(FilterToggle::LetValues);
                    true
                }
                KeyCode::Char('r') => {
                    self.toggle_filter(FilterToggle::ReverseOrder);
                    true
                }
                KeyCode::Char('d') => {
                    self.toggle_filter(FilterToggle::Definition);
                    true
                }
                KeyCode::Char('b') => {
                    self.show_goal_before = !self.show_goal_before;
                    true
                }
                _ => false,
            },
            KeyMouseEvent::Mouse(mouse) => {
                mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && self.handle_click(mouse.column, mouse.row)
            }
        }
    }

    #[allow(clippy::option_if_let_else, clippy::too_many_lines)]
    fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.click_regions.clear();

        let content = if let Some(ref error) = self.error {
            let [err, rest] =
                Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(area);
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
        let content = if let Some(def) = self
            .definition
            .as_ref()
            .filter(|_| !self.filters.hide_definition)
        {
            let [hdr, rest] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(content);
            render_definition_header(frame, hdr, def);
            rest
        } else {
            content
        };

        // Split: proof steps sidebar | main content
        let (main, steps_area) = if self.proof_steps.is_empty() {
            (content, None)
        } else {
            let [left, right] =
                Layout::horizontal([Constraint::Percentage(35), Constraint::Fill(1)])
                    .areas(content);
            (right, Some(left))
        };

        if let Some(area) = steps_area {
            render_proof_steps_sidebar(
                frame,
                area,
                &ProofStepsSidebarInput {
                    proof_steps: &self.proof_steps,
                    paperproof_steps: self.paperproof_steps.as_deref(),
                    current_step_index: self.current_step_index,
                },
            );
        }

        // Get goal_before from current paperproof step if showing
        let goal_before = if self.show_goal_before {
            self.paperproof_steps
                .as_ref()
                .and_then(|steps| steps.get(self.current_step_index))
                .map(|step| &step.goal_before)
        } else {
            None
        };

        // Split main: hypotheses | divider | goal_before (optional) | goals
        let (hyps, div, goal_before_area, goals) = if goal_before.is_some() {
            let [hyps, div, goal_before_area, goals] = Layout::vertical([
                Constraint::Percentage(45),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Fill(1),
            ])
            .areas(main);
            (hyps, div, Some(goal_before_area), goals)
        } else {
            let [hyps, div, goals] = Layout::vertical([
                Constraint::Percentage(55),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .areas(main);
            (hyps, div, None, goals)
        };

        // Collect depends_on from current proof step
        let depends_on: HashSet<String> = self
            .proof_steps
            .get(self.current_step_index)
            .map(|step| step.depends_on.iter().cloned().collect())
            .unwrap_or_default();

        // Collect spawned goal usernames from current paperproof step
        let spawned_goal_ids: HashSet<String> = self
            .paperproof_steps
            .as_ref()
            .and_then(|steps| steps.get(self.current_step_index))
            .map(|step| step.spawned_goals.iter().map(|g| g.username.clone()).collect())
            .unwrap_or_default();

        // Update and render hypothesis section
        self.hyp_section.update(HypSectionInput {
            goals: self.goals.clone(),
            filters: self.filters,
            depends_on,
            selection: self.current_selection(),
        });
        self.hyp_section.render(frame, hyps);
        self.click_regions
            .extend(self.hyp_section.click_regions().iter().cloned());

        render_divider(frame, div, None);

        // Render goal_before if toggled
        if let (Some(area), Some(goal_before)) = (goal_before_area, goal_before) {
            render_goal_before(frame, area, goal_before);
        }

        // Update and render goal section
        self.goal_section.update(GoalSectionInput {
            goals: self.goals.clone(),
            selection: self.current_selection(),
            spawned_goal_ids,
        });
        self.goal_section.render(frame, goals);
        self.click_regions
            .extend(self.goal_section.click_regions().iter().cloned());
    }
}
