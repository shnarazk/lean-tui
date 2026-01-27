//! Deduction Tree mode - Paperproof tree visualization.

use std::iter;

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::Stylize,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use super::{Backend, Mode};
use crate::{
    lean_rpc::{Goal, PaperproofStep},
    tui::components::{
        hypothesis_indices, render_definition_header, render_tree_view, ClickRegion, Component,
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
    selected_index: Option<usize>,
    click_regions: Vec<ClickRegion>,
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

    fn reset_selection(&mut self) {
        self.selected_index = (!self.selectable_items().is_empty()).then_some(0);
    }

    fn select_previous(&mut self) {
        let count = self.selectable_items().len();
        if count == 0 {
            return;
        }
        self.selected_index = Some(self.selected_index.map_or(0, |i| i.saturating_sub(1)));
    }

    fn select_next(&mut self) {
        let count = self.selectable_items().len();
        if count == 0 {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) if i < count - 1 => i + 1,
            Some(i) => i,
            None => 0,
        });
    }

    fn handle_click(&mut self, x: u16, y: u16) -> bool {
        let clicked_item = self.find_click_region(x, y).map(|r| r.item);
        let Some(item) = clicked_item else {
            return false;
        };

        let items = self.selectable_items();
        if let Some(idx) = items.iter().position(|i| *i == item) {
            self.selected_index = Some(idx);
            return true;
        }
        false
    }

    fn find_click_region(&self, x: u16, y: u16) -> Option<&ClickRegion> {
        self.click_regions.iter().find(|region| {
            region.area.x <= x
                && x < region.area.x + region.area.width
                && region.area.y <= y
                && y < region.area.y + region.area.height
        })
    }
}

impl Component for DeductionTreeMode {
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
            self.reset_selection();
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
                _ => false,
            },
            KeyMouseEvent::Mouse(mouse) => {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    self.handle_click(mouse.column, mouse.row)
                } else {
                    false
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.click_regions.clear();

        // Handle error display
        #[allow(clippy::option_if_let_else)]
        let content_area = if let Some(ref error) = self.error {
            let [error_area, rest] =
                Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(area);
            frame.render_widget(
                Paragraph::new(format!("Error: {error}")).fg(Color::Red),
                error_area,
            );
            rest
        } else {
            area
        };

        if self.goals.is_empty() {
            frame.render_widget(
                Paragraph::new("No goals").style(Style::new().fg(Color::DarkGray)),
                content_area,
            );
            return;
        }

        // Definition header (always shown if available)
        let content_area = if let Some(def) = self.definition.as_ref() {
            let [header_area, rest] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(content_area);
            render_definition_header(frame, header_area, def);
            rest
        } else {
            content_area
        };

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
        let items = self.selectable_items();
        self.selected_index.and_then(|i| items.get(i).copied())
    }
}
