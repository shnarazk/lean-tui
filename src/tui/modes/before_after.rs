//! Before/After mode - three-column temporal comparison view.

use std::iter;

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    Frame,
};

use super::Mode;
use crate::{
    lean_rpc::Goal,
    tui::widgets::{
        goals_column::{GoalsColumn, GoalsColumnState},
        hypothesis_indices,
        render_helpers::render_error,
        selection::SelectionState,
        FilterToggle, HypothesisFilters, InteractiveComponent, KeyMouseEvent, Selection,
    },
    tui_ipc::{DefinitionInfo, ProofDag},
};

/// Input for updating the Before/After mode.
pub struct BeforeAfterModeInput {
    pub previous_goals: Option<Vec<Goal>>,
    pub current_goals: Vec<Goal>,
    pub next_goals: Option<Vec<Goal>>,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub proof_dag: Option<ProofDag>,
}

/// Before/After display mode - temporal comparison of goal states.
pub struct BeforeAfterMode {
    previous_goals: Option<Vec<Goal>>,
    current_goals: Vec<Goal>,
    next_goals: Option<Vec<Goal>>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    /// Current node ID in the DAG (for building selections).
    current_node_id: Option<u32>,
    /// Name of the goal the cursor's tactic is working on.
    active_goal_name: Option<String>,
    filters: HypothesisFilters,
    selection: SelectionState,
    show_previous: bool,
    show_next: bool,
    previous_column_state: GoalsColumnState,
    current_column_state: GoalsColumnState,
    next_column_state: GoalsColumnState,
}

impl Default for BeforeAfterMode {
    fn default() -> Self {
        Self {
            previous_goals: None,
            current_goals: Vec::new(),
            next_goals: None,
            definition: None,
            error: None,
            current_node_id: None,
            active_goal_name: None,
            filters: HypothesisFilters::default(),
            selection: SelectionState::default(),
            show_previous: true, // Show previous column by default
            show_next: false,
            previous_column_state: GoalsColumnState::default(),
            current_column_state: GoalsColumnState::default(),
            next_column_state: GoalsColumnState::default(),
        }
    }
}

impl BeforeAfterMode {
    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

    /// Whether the previous column should be shown.
    pub const fn show_previous(&self) -> bool {
        self.show_previous
    }

    /// Whether the next column should be shown.
    pub const fn show_next(&self) -> bool {
        self.show_next
    }

    fn selectable_items(&self) -> Vec<Selection> {
        let Some(node_id) = self.current_node_id else {
            return Vec::new();
        };

        self.current_goals
            .iter()
            .enumerate()
            .flat_map(|(goal_idx, goal)| {
                let hyp_items = hypothesis_indices(goal.hyps.len(), self.filters.reverse_order)
                    .filter(|&hyp_idx| self.filters.should_show(&goal.hyps[hyp_idx]))
                    .map(move |hyp_idx| Selection::Hyp { node_id, hyp_idx });

                hyp_items.chain(iter::once(Selection::Goal { node_id, goal_idx }))
            })
            .collect()
    }
}

impl InteractiveComponent for BeforeAfterMode {
    type Input = BeforeAfterModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        let goals_changed = self.current_goals != input.current_goals;
        self.previous_goals = input.previous_goals;
        self.current_goals = input.current_goals;
        self.next_goals = input.next_goals;
        self.definition = input.definition;
        self.error = input.error;
        let current_node_id = input.proof_dag.as_ref().and_then(|dag| dag.current_node);
        let current_node = current_node_id.and_then(|id| input.proof_dag.as_ref()?.get(id));
        self.current_node_id = current_node_id;
        self.active_goal_name = current_node
            .and_then(|node| node.state_before.goals.first())
            .and_then(|g| g.username.as_str().map(String::from));
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
                KeyCode::Char('p') => {
                    self.show_previous = !self.show_previous;
                    true
                }
                KeyCode::Char('n') => {
                    self.show_next = !self.show_next;
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

        // Three-column layout
        let has_prev = self.previous_goals.is_some() && self.show_previous;
        let has_next = self.next_goals.is_some() && self.show_next;

        let constraints = match (has_prev, has_next) {
            (true, true) => vec![
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ],
            (true, false) => vec![Constraint::Percentage(35), Constraint::Percentage(65)],
            (false, true) => vec![Constraint::Percentage(65), Constraint::Percentage(35)],
            (false, false) => vec![Constraint::Percentage(100)],
        };

        let columns = Layout::horizontal(constraints).split(content_area);
        let mut col_idx = 0;

        let selection = self.current_selection();

        // Previous column
        if let Some(ref goals) = self.previous_goals {
            if self.show_previous {
                frame.render_stateful_widget(
                    GoalsColumn::new(
                        "Previous",
                        goals,
                        self.filters,
                        None,
                        false,
                        None,
                        self.active_goal_name.as_deref(),
                    ),
                    columns[col_idx],
                    &mut self.previous_column_state,
                );
                col_idx += 1;
            }
        }

        // Current column (always shown)
        frame.render_stateful_widget(
            GoalsColumn::new(
                "Current",
                &self.current_goals,
                self.filters,
                selection,
                true,
                self.current_node_id,
                self.active_goal_name.as_deref(),
            ),
            columns[col_idx],
            &mut self.current_column_state,
        );
        // Register click regions from current column
        for region in self.current_column_state.click_regions() {
            self.selection.add_region(region.area, region.selection);
        }
        col_idx += 1;

        // Next column
        if let Some(ref goals) = self.next_goals {
            if self.show_next {
                frame.render_stateful_widget(
                    GoalsColumn::new(
                        "Next",
                        goals,
                        self.filters,
                        None,
                        false,
                        None,
                        self.active_goal_name.as_deref(),
                    ),
                    columns[col_idx],
                    &mut self.next_column_state,
                );
            }
        }
    }
}

impl Mode for BeforeAfterMode {
    type Model = BeforeAfterModeInput;

    const NAME: &'static str = "Before/After";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[
        ("p", "prev"),
        ("n", "next"),
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

    fn current_selection(&self) -> Option<Selection> {
        self.selection
            .current_selection(&self.selectable_items())
            .copied()
    }
}
