//! Open Goal List mode - displays hypotheses and goal targets in a navigable
//! list.

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{layout::Rect, Frame};

use super::Mode;
use crate::{
    lean_rpc::{ProofDag, ProofState},
    tui::{
        app::DefinitionInfo,
        widgets::{
            hypothesis_indices, open_goal_list::OpenGoalList, render_helpers::render_error,
            selection::SelectionState, FilterToggle, HypothesisFilters, InteractiveComponent,
            KeyMouseEvent, Selection,
        },
    },
};

/// Input for updating the Open Goal List mode.
pub struct PlainListInput {
    pub state: ProofState,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub proof_dag: Option<ProofDag>,
}

/// Open Goal List display mode - navigable list of open goals with hypotheses.
#[derive(Default)]
pub struct PlainList {
    /// Current proof state.
    state: ProofState,
    /// Current node ID in the DAG (for building selections).
    current_node_id: Option<u32>,
    /// Name of the goal the cursor's tactic is working on.
    active_goal_name: Option<String>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    filters: HypothesisFilters,
    selection: SelectionState,
}

impl PlainList {
    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

    fn selectable_items(&self) -> Vec<Selection> {
        let Some(node_id) = self.current_node_id else {
            return Vec::new();
        };

        let hyp_count = self.state.hypotheses.len();
        let goal_count = self.state.goals.len();

        // All hypotheses (filtered) followed by all goals
        let hyp_items = hypothesis_indices(hyp_count, self.filters.reverse_order)
            .filter(|&i| self.should_show_hypothesis(i))
            .map(move |hyp_idx| Selection::Hyp { node_id, hyp_idx });

        let goal_items = (0..goal_count).map(move |goal_idx| Selection::Goal { node_id, goal_idx });

        hyp_items.chain(goal_items).collect()
    }

    fn should_show_hypothesis(&self, idx: usize) -> bool {
        let Some(h) = self.state.hypotheses.get(idx) else {
            return false;
        };
        // Apply filters based on hypothesis properties
        if self.filters.hide_instances && h.is_instance {
            return false;
        }
        // Note: hide_inaccessible checks name prefix, is_proof maps to that heuristic
        if self.filters.hide_inaccessible && h.is_proof {
            return false;
        }
        true
    }
}

impl InteractiveComponent for PlainList {
    type Input = PlainListInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        // Extract current node ID from DAG
        let current_node_id = input.proof_dag.as_ref().and_then(|dag| dag.current_node);
        let current_node = current_node_id.and_then(|id| input.proof_dag.as_ref()?.get(id));

        let state_changed = self.state.goals.len() != input.state.goals.len()
            || self.state.hypotheses.len() != input.state.hypotheses.len();

        self.current_node_id = current_node_id;
        self.active_goal_name = current_node
            .and_then(|node| node.state_before.goals.first())
            .and_then(|g| g.username.as_str().map(String::from));
        self.state = input.state;
        self.definition = input.definition;
        self.error = input.error;

        if state_changed {
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

        // Render open goal list and collect click regions
        let goal_list = OpenGoalList::new(
            &self.state,
            self.current_selection(),
            self.filters,
            self.current_node_id,
            self.active_goal_name.as_deref(),
        );
        let click_regions = goal_list.render_to_frame(frame, content_area);

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
                region.selection,
            );
        }
    }
}

impl Mode for PlainList {
    type Model = PlainListInput;

    const NAME: &'static str = "Plain list";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] =
        &[("i", "inst"), ("a", "access"), ("l", "let"), ("r", "rev")];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[
        FilterToggle::Instances,
        FilterToggle::Inaccessible,
        FilterToggle::LetValues,
        FilterToggle::ReverseOrder,
    ];

    fn current_selection(&self) -> Option<Selection> {
        self.selection
            .current_selection(&self.selectable_items())
            .copied()
    }
}
