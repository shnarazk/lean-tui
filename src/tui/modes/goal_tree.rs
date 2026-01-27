//! Goal Tree mode - displays hypotheses and goal targets in a navigable tree.

use std::iter;

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::Stylize,
    style::Color,
    widgets::Paragraph,
    Frame,
};

use super::Mode;
use crate::{
    lean_rpc::Goal,
    tui::components::{
        goal_tree::GoalTree, hypothesis_indices, ClickRegion, Component, FilterToggle,
        HypothesisFilters, KeyMouseEvent, SelectableItem,
    },
    tui_ipc::{CaseSplitInfo, DefinitionInfo},
};

/// Input for updating the Goal Tree mode.
pub struct GoalTreeModeInput {
    pub goals: Vec<Goal>,
    pub definition: Option<DefinitionInfo>,
    pub case_splits: Vec<CaseSplitInfo>,
    pub error: Option<String>,
}

/// Goal Tree display mode - navigable hypothesis and goal tree.
#[derive(Default)]
pub struct GoalTreeMode {
    goals: Vec<Goal>,
    definition: Option<DefinitionInfo>,
    case_splits: Vec<CaseSplitInfo>,
    error: Option<String>,
    filters: HypothesisFilters,
    selected_index: Option<usize>,
    click_regions: Vec<ClickRegion>,
}

impl GoalTreeMode {
    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

    pub const fn toggle_filter(&mut self, filter: FilterToggle) {
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

impl Component for GoalTreeMode {
    type Input = GoalTreeModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        let goals_changed = self.goals != input.goals;
        self.goals = input.goals;
        self.definition = input.definition;
        self.case_splits = input.case_splits;
        self.error = input.error;
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

        // Render goal tree
        let mut tree = GoalTree::new(
            &self.goals,
            self.definition.as_ref(),
            &self.case_splits,
            self.current_selection(),
            self.filters,
        );
        tree.render(frame, content_area);

        // Collect click regions, adjusting for error offset
        let y_offset = if self.error.is_some() { 2 } else { 0 };
        for region in tree.click_regions() {
            self.click_regions.push(ClickRegion {
                area: Rect::new(
                    region.area.x,
                    region.area.y + y_offset,
                    region.area.width,
                    region.area.height,
                ),
                item: region.item,
            });
        }
    }
}

impl Mode for GoalTreeMode {
    type Model = GoalTreeModeInput;

    const NAME: &'static str = "Goal Tree";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[
        ("i", "inst"),
        ("t", "type"),
        ("a", "access"),
        ("l", "let"),
        ("r", "rev"),
        ("d", "def"),
    ];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[
        FilterToggle::Instances,
        FilterToggle::Types,
        FilterToggle::Inaccessible,
        FilterToggle::LetValues,
        FilterToggle::ReverseOrder,
        FilterToggle::Definition,
    ];

    fn current_selection(&self) -> Option<SelectableItem> {
        let items = self.selectable_items();
        self.selected_index.and_then(|i| items.get(i).copied())
    }
}
