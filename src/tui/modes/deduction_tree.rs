//! Deduction Tree mode - Paperproof tree visualization.

use std::iter;

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use super::{Backend, Mode};
use crate::{
    lean_rpc::{Goal, PaperproofStep},
    tui::widgets::{
        hypothesis_indices,
        interactive_widget::InteractiveWidget,
        render_helpers::{render_error, render_no_goals},
        selection::SelectionState,
        tree_view::render_tree_view,
        FilterToggle, HypothesisFilters, KeyMouseEvent, SelectableItem,
    },
    tui_ipc::DefinitionInfo,
};

/// Input for updating the Deduction Tree mode.
pub struct DeductionTreeModeInput {
    pub goals: Vec<Goal>,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub current_step_index: usize,
    pub paperproof_steps: Option<Vec<PaperproofStep>>,
}

/// Deduction Tree display mode - Paperproof tree visualization.
#[derive(Default)]
pub struct DeductionTreeMode {
    goals: Vec<Goal>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    current_step_index: usize,
    paperproof_steps: Option<Vec<PaperproofStep>>,
    filters: HypothesisFilters,
    selection: SelectionState,
}

impl DeductionTreeMode {
    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

    fn selectable_items(&self) -> Vec<SelectableItem> {
        self.goals
            .iter()
            .enumerate()
            .flat_map(|(goal_idx, goal)| {
                let hyp_items = hypothesis_indices(goal.hyps.len(), self.filters.reverse_order)
                    .filter(|&hyp_idx| self.filters.should_show(&goal.hyps[hyp_idx]))
                    .map(move |hyp_idx| SelectableItem::Hypothesis { goal_idx, hyp_idx });

                hyp_items.chain(iter::once(SelectableItem::GoalTarget { goal_idx }))
            })
            .collect()
    }
}

impl InteractiveWidget for DeductionTreeMode {
    type Input = DeductionTreeModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        let goals_changed = self.goals != input.goals;
        self.goals = input.goals;
        self.definition = input.definition;
        self.error = input.error;
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

        let content_area = render_error(frame, area, self.error.as_deref());

        if self.goals.is_empty() {
            render_no_goals(frame, content_area);
            return;
        }

        // Render tree view
        if let Some(ref steps) = self.paperproof_steps {
            render_tree_view(frame, content_area, steps, self.current_step_index);
        } else {
            frame.render_widget(
                Paragraph::new("Tree view requires Paperproof data")
                    .style(Style::new().fg(Color::DarkGray)),
                content_area,
            );
        }
    }
}

impl Mode for DeductionTreeMode {
    type Model = DeductionTreeModeInput;

    const NAME: &'static str = "Deduction Tree";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[];
    const BACKENDS: &'static [Backend] = &[Backend::Paperproof];

    fn current_selection(&self) -> Option<SelectableItem> {
        self.selection
            .current_selection(&self.selectable_items())
            .copied()
    }
}
