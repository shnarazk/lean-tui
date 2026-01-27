//! Goal Tree mode - displays hypotheses and goal targets in a navigable tree.

use std::iter;

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{layout::Rect, Frame};

use super::{Backend, Mode};
use crate::{
    lean_rpc::Goal,
    tui::widgets::{
        goal_tree::GoalTree, hypothesis_indices, interactive_widget::InteractiveWidget,
        render_helpers::render_error, selection::SelectionState, FilterToggle, HypothesisFilters,
        KeyMouseEvent, SelectableItem,
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
    selection: SelectionState,
}

impl GoalTreeMode {
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

impl InteractiveWidget for GoalTreeMode {
    type Input = GoalTreeModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        let goals_changed = self.goals != input.goals;
        self.goals = input.goals;
        self.definition = input.definition;
        self.case_splits = input.case_splits;
        self.error = input.error;
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

        // Render goal tree and collect click regions
        let tree = GoalTree::new(
            &self.goals,
            &self.case_splits,
            self.current_selection(),
            self.filters,
        );
        let click_regions = tree.render_to_frame(frame, content_area);

        // Adjust click regions for error offset and add to selection
        let y_offset = if self.error.is_some() { 2 } else { 0 };
        for region in click_regions {
            self.selection.add_region(
                Rect::new(
                    region.area.x,
                    region.area.y + y_offset,
                    region.area.width,
                    region.area.height,
                ),
                region.item,
            );
        }
    }
}

impl Mode for GoalTreeMode {
    type Model = GoalTreeModeInput;

    const NAME: &'static str = "Goal Tree";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] =
        &[("i", "inst"), ("a", "access"), ("l", "let"), ("r", "rev")];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[
        FilterToggle::Instances,
        FilterToggle::Inaccessible,
        FilterToggle::LetValues,
        FilterToggle::ReverseOrder,
    ];
    const BACKENDS: &'static [Backend] = &[Backend::LeanRpc];

    fn current_selection(&self) -> Option<SelectableItem> {
        self.selection
            .current_selection(&self.selectable_items())
            .copied()
    }
}
