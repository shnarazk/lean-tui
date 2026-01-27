//! Goal Tree mode - displays hypotheses and goal targets in a navigable tree.

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{layout::Rect, Frame};

use super::{Backend, Mode};
use crate::{
    lean_rpc::Goal,
    tui::widgets::{
        goal_tree::GoalTree, hypothesis_indices, render_helpers::render_error,
        selection::SelectionState, FilterToggle, HypothesisFilters, InteractiveComponent,
        KeyMouseEvent, Selection,
    },
    tui_ipc::{DefinitionInfo, ProofDag, ProofState},
};

/// Input for updating the Goal Tree mode.
pub struct GoalTreeModeInput {
    pub goals: Vec<Goal>,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub proof_dag: Option<ProofDag>,
}

/// Goal Tree display mode - navigable hypothesis and goal tree.
#[derive(Default)]
pub struct GoalTreeMode {
    /// Current proof state (from DAG or converted from goals).
    state: ProofState,
    /// Current node ID in the DAG (for building selections).
    current_node_id: Option<u32>,
    /// Fallback goals (used when DAG not available).
    goals: Vec<Goal>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    filters: HypothesisFilters,
    selection: SelectionState,
}

impl GoalTreeMode {
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

impl InteractiveComponent for GoalTreeMode {
    type Input = GoalTreeModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        // Extract current node ID and state from DAG
        let current_node_id = input.proof_dag.as_ref().and_then(|dag| dag.current_node);
        let new_state = current_node_id
            .and_then(|id| dag_state(input.proof_dag.as_ref(), id))
            .unwrap_or_else(|| ProofState::from_goals(&input.goals));

        let state_changed = self.state.goals.len() != new_state.goals.len()
            || self.state.hypotheses.len() != new_state.hypotheses.len();

        self.current_node_id = current_node_id;
        self.state = new_state;
        self.goals = input.goals;
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

        // Render goal tree and collect click regions
        let tree = GoalTree::new(
            &self.goals,
            self.current_selection(),
            self.filters,
            self.current_node_id,
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
                region.selection,
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

    fn current_selection(&self) -> Option<Selection> {
        self.selection
            .current_selection(&self.selectable_items())
            .copied()
    }
}

/// Extract state from DAG's current node.
fn dag_state(proof_dag: Option<&ProofDag>, node_id: u32) -> Option<ProofState> {
    proof_dag?.get(node_id).map(|node| node.state_after.clone())
}
