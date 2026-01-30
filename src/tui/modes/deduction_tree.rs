//! Deduction Tree mode - semantic tree visualization.

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};
use tracing::debug;

use super::Mode;
use crate::{
    lean_rpc::{ProofDag, ProofState},
    tui::widgets::{
        render_helpers::{render_error, render_no_goals},
        semantic_tableau::{
            navigation::{find_nearest_in_direction, Direction},
            SemanticTableauLayout, SemanticTableauState,
        },
        FilterToggle, HypothesisFilters, InteractiveComponent, KeyMouseEvent, Selection,
    },
    tui_ipc::DefinitionInfo,
};

/// Input for updating the Deduction Tree mode.
pub struct DeductionTreeModeInput {
    pub state: ProofState,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub proof_dag: Option<ProofDag>,
}

/// Deduction Tree display mode - semantic tree visualization.
pub struct SemanticTableau {
    state: ProofState,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    proof_dag: Option<ProofDag>,
    filters: HypothesisFilters,
    /// Flat index into `tree_selectable_items()`.
    selected_idx: Option<usize>,
    /// When true, tree renders top-down (parent above children).
    tree_top_down: bool,
    /// State for the semantic tableau widget.
    tableau_state: SemanticTableauState,
}

impl Default for SemanticTableau {
    fn default() -> Self {
        Self {
            state: ProofState::default(),
            definition: None,
            error: None,
            proof_dag: None,
            filters: HypothesisFilters::default(),
            selected_idx: None,
            tree_top_down: true,
            tableau_state: SemanticTableauState::default(),
        }
    }
}

impl SemanticTableau {
    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

    /// Build list of all selectable items in the tree from `proof_dag`.
    fn tree_selectable_items(&self) -> Vec<Selection> {
        let Some(dag) = &self.proof_dag else {
            return Vec::new();
        };
        if dag.is_empty() {
            return Vec::new();
        }

        let mut items = Vec::new();

        // Initial hypotheses from the initial_state
        for (idx, h) in dag.initial_state.hypotheses.iter().enumerate() {
            if !h.is_proof {
                items.push(Selection::InitialHyp { hyp_idx: idx });
            }
        }

        // For each node: new hypotheses and goals
        for node in dag.dfs_iter() {
            let node_id = node.id;

            // New hypotheses introduced by this step
            for &hyp_idx in &node.new_hypotheses {
                items.push(Selection::Hyp { node_id, hyp_idx });
            }

            // Goals after this step
            for (goal_idx, _) in node.state_after.goals.iter().enumerate() {
                items.push(Selection::Goal { node_id, goal_idx });
            }
        }

        // Theorem (final goal)
        items.push(Selection::Theorem);

        items
    }

    fn current_tree_selection(&self) -> Option<Selection> {
        let items = self.tree_selectable_items();
        self.selected_idx.and_then(|idx| items.get(idx).copied())
    }

    fn select_by_selection(&mut self, sel: Selection) {
        let items = self.tree_selectable_items();
        if let Some(idx) = items.iter().position(|s| *s == sel) {
            self.selected_idx = Some(idx);
        }
    }

    /// Get the active goal selection (first goal of the current node).
    fn active_goal_selection(&self) -> Option<Selection> {
        let dag = self.proof_dag.as_ref()?;
        let current_node_id = dag.current_node?;
        Some(Selection::Goal {
            node_id: current_node_id,
            goal_idx: 0,
        })
    }

    fn move_in_direction(&mut self, direction: Direction) -> bool {
        // If nothing is selected, start at the active goal
        if self.selected_idx.is_none() {
            debug!("No selection, starting at active goal");
            if let Some(sel) = self.active_goal_selection() {
                self.select_by_selection(sel);
                return true;
            }
        }

        let current = self.current_tree_selection();
        let Some(current_sel) = current else {
            debug!("No current selection found");
            return false;
        };

        let navigation_regions = self.tableau_state.proof.navigation_regions();
        let items = self.tree_selectable_items();

        debug!(
            ?current_sel,
            ?direction,
            nav_regions = navigation_regions.len(),
            selectable_items = items.len(),
            "move_in_direction called"
        );

        let result = find_nearest_in_direction(navigation_regions, current_sel, direction);
        debug!(?result, "Navigation result in move_in_direction");

        result.is_some_and(|sel| {
            let found_in_items = items.contains(&sel);
            debug!(?sel, found_in_items, "Attempting to select");
            self.select_by_selection(sel);
            true
        })
    }
}

impl InteractiveComponent for SemanticTableau {
    type Input = DeductionTreeModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        // Detect changes to proof tree or goals
        let old_count = self.proof_dag.as_ref().map_or(0, ProofDag::len);
        let new_count = input.proof_dag.as_ref().map_or(0, ProofDag::len);
        let old_current = self.proof_dag.as_ref().and_then(|dag| dag.current_node);
        let new_current = input.proof_dag.as_ref().and_then(|dag| dag.current_node);

        let tree_changed = old_count != new_count || old_current != new_current;
        let state_changed = self.state.goals.len() != input.state.goals.len()
            || self.state.hypotheses.len() != input.state.hypotheses.len();

        // Update state when current node changes
        self.tableau_state.update_current_node(new_current);

        self.state = input.state;
        self.definition = input.definition;
        self.error = input.error;
        self.proof_dag = input.proof_dag;

        // Auto-select active goal when tree or goals change
        if tree_changed || state_changed {
            if let Some(sel) = self.active_goal_selection() {
                self.select_by_selection(sel);
            } else {
                self.selected_idx = None;
            }
        }
    }

    fn handle_event(&mut self, event: Self::Event) -> bool {
        match event {
            KeyMouseEvent::Key(key) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.move_in_direction(Direction::Down),
                KeyCode::Char('k') | KeyCode::Up => self.move_in_direction(Direction::Up),
                KeyCode::Char('h') | KeyCode::Left => self.move_in_direction(Direction::Left),
                KeyCode::Char('l') | KeyCode::Right => self.move_in_direction(Direction::Right),
                KeyCode::Char('t') => {
                    self.tree_top_down = !self.tree_top_down;
                    true
                }
                _ => false,
            },
            KeyMouseEvent::Mouse(mouse) => {
                let is_click = mouse.kind == MouseEventKind::Down(MouseButton::Left);
                let clicked = is_click
                    .then(|| self.tableau_state.find_click_at(mouse.column, mouse.row))
                    .flatten();
                if let Some(sel) = clicked {
                    self.select_by_selection(sel);
                    return true;
                }
                false
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let content_area = render_error(frame, area, self.error.as_deref());

        if self.state.goals.is_empty() {
            render_no_goals(frame, content_area);
            return;
        }

        if let Some(ref dag) = self.proof_dag {
            let widget = SemanticTableauLayout::new(
                dag,
                self.tree_top_down,
                self.current_tree_selection(),
                &self.state,
            );
            frame.render_stateful_widget(widget, content_area, &mut self.tableau_state);
        } else {
            frame.render_widget(
                Paragraph::new("No proof steps available").style(Style::new().fg(Color::DarkGray)),
                content_area,
            );
        }
    }
}

impl Mode for SemanticTableau {
    type Model = DeductionTreeModeInput;

    const NAME: &'static str = "Semantic tableau";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[("hjkl", "nav")];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[];

    fn current_selection(&self) -> Option<Selection> {
        self.current_tree_selection()
    }
}
