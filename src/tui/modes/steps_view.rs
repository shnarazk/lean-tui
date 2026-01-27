//! Steps mode - sidebar with proof steps plus hypotheses and goals sections.

use std::{collections::HashSet, iter};

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    Frame,
};

use super::{Backend, Mode};
use crate::{
    lean_rpc::{Goal, PaperproofStep},
    tui::components::{
        divider, hypothesis_indices, render_error, render_goal_before, render_no_goals, Component,
        FilterToggle, GoalSection, GoalSectionInput, HypSection, HypSectionInput,
        HypothesisFilters, KeyMouseEvent, ProofStepsSidebar, SelectableItem, SelectionState,
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
    selection: SelectionState,
    show_goal_before: bool,
}

impl StepsMode {
    fn selectable_items(&self) -> Vec<SelectableItem> {
        self.goals
            .iter()
            .enumerate()
            .flat_map(|(goal_idx, goal)| {
                let hyps = hypothesis_indices(goal.hyps.len(), self.filters.reverse_order)
                    .filter(|&i| self.filters.should_show(&goal.hyps[i]))
                    .map(move |i| SelectableItem::Hypothesis {
                        goal_idx,
                        hyp_idx: i,
                    });
                hyps.chain(iter::once(SelectableItem::GoalTarget { goal_idx }))
            })
            .collect()
    }

    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

    fn layout_with_sidebar(&self, area: Rect) -> (Rect, Option<Rect>) {
        if self.proof_steps.is_empty() {
            (area, None)
        } else {
            let [sidebar, main] = Layout::horizontal([
                Constraint::Min(20), // Sidebar needs at least 20 chars
                Constraint::Fill(1), // Main content takes remaining space
            ])
            .flex(Flex::Start)
            .areas(area);
            (main, Some(sidebar))
        }
    }

    fn layout_main(area: Rect, has_goal_before: bool) -> MainLayout {
        if has_goal_before {
            let [hyps, div, goal_before, goals] = Layout::vertical([
                Constraint::Fill(1),   // Hypotheses expand
                Constraint::Length(1), // Divider is always 1 line
                Constraint::Min(2),    // Goal before needs at least 2 lines
                Constraint::Fill(1),   // Goals expand equally
            ])
            .areas(area);
            MainLayout {
                hyps,
                div,
                goal_before: Some(goal_before),
                goals,
            }
        } else {
            let [hyps, div, goals] = Layout::vertical([
                Constraint::Fill(1),   // Hypotheses expand
                Constraint::Length(1), // Divider is always 1 line
                Constraint::Fill(1),   // Goals expand equally
            ])
            .areas(area);
            MainLayout {
                hyps,
                div,
                goal_before: None,
                goals,
            }
        }
    }
}

struct MainLayout {
    hyps: Rect,
    div: Rect,
    goal_before: Option<Rect>,
    goals: Rect,
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
            self.selection.reset(self.selectable_items().len());
        }
    }

    fn handle_event(&mut self, event: Self::Event) -> bool {
        let items = self.selectable_items();
        match event {
            KeyMouseEvent::Key(key) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.selection.select_next(items.len());
                    true
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.selection.select_previous(items.len());
                    true
                }
                KeyCode::Char('i') => {
                    self.filters.toggle(FilterToggle::Instances);
                    true
                }
                KeyCode::Char('a') => {
                    self.filters.toggle(FilterToggle::Inaccessible);
                    true
                }
                KeyCode::Char('l') => {
                    self.filters.toggle(FilterToggle::LetValues);
                    true
                }
                KeyCode::Char('r') => {
                    self.filters.toggle(FilterToggle::ReverseOrder);
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
                    && self.selection.handle_click(mouse.column, mouse.row, &items)
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.selection.clear_regions();

        let content = render_error(frame, area, self.error.as_deref());

        if self.goals.is_empty() {
            render_no_goals(frame, content);
            return;
        }

        // Layout: optional sidebar | main content
        let (main, sidebar) = self.layout_with_sidebar(content);

        // Render sidebar if present
        if let Some(sidebar_area) = sidebar {
            frame.render_widget(
                ProofStepsSidebar::new(
                    &self.proof_steps,
                    self.paperproof_steps.as_deref(),
                    self.current_step_index,
                ),
                sidebar_area,
            );
        }

        // Get goal_before from current paperproof step if showing
        let goal_before = self
            .show_goal_before
            .then(|| {
                self.paperproof_steps
                    .as_ref()
                    .and_then(|steps| steps.get(self.current_step_index))
                    .map(|step| &step.goal_before)
            })
            .flatten();

        // Layout main area
        let layout = Self::layout_main(main, goal_before.is_some());

        // Collect context data
        let depends_on: HashSet<String> = self
            .proof_steps
            .get(self.current_step_index)
            .map(|step| step.depends_on.iter().cloned().collect())
            .unwrap_or_default();

        let spawned_goal_ids: HashSet<String> = self
            .paperproof_steps
            .as_ref()
            .and_then(|steps| steps.get(self.current_step_index))
            .map(|step| {
                step.spawned_goals
                    .iter()
                    .map(|g| g.username.clone())
                    .collect()
            })
            .unwrap_or_default();

        // Render hypothesis section
        self.hyp_section.update(HypSectionInput {
            goals: self.goals.clone(),
            filters: self.filters,
            depends_on,
            selection: self.current_selection(),
        });
        self.hyp_section.render(frame, layout.hyps);
        for region in self.hyp_section.click_regions() {
            self.selection.add_region(region.area, region.item);
        }

        // Render divider
        frame.render_widget(divider(), layout.div);

        // Render goal_before if toggled
        if let (Some(area), Some(goal_before)) = (layout.goal_before, goal_before) {
            render_goal_before(frame, area, goal_before);
        }

        // Render goal section
        self.goal_section.update(GoalSectionInput {
            goals: self.goals.clone(),
            selection: self.current_selection(),
            spawned_goal_ids,
        });
        self.goal_section.render(frame, layout.goals);
        for region in self.goal_section.click_regions() {
            self.selection.add_region(region.area, region.item);
        }
    }
}

impl Mode for StepsMode {
    type Model = StepsModeInput;

    const NAME: &'static str = "Steps";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[
        ("b", "before"),
        ("i", "inst"),
        ("a", "access"),
        ("l", "let"),
        ("r", "rev"),
    ];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[
        FilterToggle::Instances,
        FilterToggle::Inaccessible,
        FilterToggle::LetValues,
        FilterToggle::ReverseOrder,
    ];
    const BACKENDS: &'static [Backend] = &[Backend::Paperproof, Backend::TreeSitter];

    fn current_selection(&self) -> Option<SelectableItem> {
        self.selection
            .current_selection(&self.selectable_items())
            .copied()
    }
}
