//! Steps mode - sidebar with proof steps plus hypotheses and goals sections.

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    Frame,
};

use super::Mode;
use crate::{
    lean_rpc::Goal,
    tui::widgets::{
        goal_section::{GoalSection, GoalSectionState},
        hyp_section::{HypSection, HypSectionState},
        hypothesis_indices,
        proof_steps_sidebar::{ProofStepsSidebar, ProofStepsSidebarState},
        render_helpers::{render_error, render_no_goals},
        selection::SelectionState,
        tactic_row::divider,
        theme::FocusedPane,
        FilterToggle, HypothesisFilters, InteractiveComponent, InteractiveStatefulWidget,
        KeyMouseEvent, Selection,
    },
    tui_ipc::{DefinitionInfo, ProofDag, ProofDagNode},
};

/// Input for updating the Steps mode.
pub struct StepsModeInput {
    pub goals: Vec<Goal>,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub proof_dag: Option<ProofDag>,
}

/// Steps display mode - sidebar + hypotheses + goals.
#[derive(Default)]
pub struct StepsMode {
    goals: Vec<Goal>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    proof_dag: Option<ProofDag>,
    /// Current node ID in the DAG (for building selections).
    current_node_id: Option<u32>,
    hyp_section_state: HypSectionState,
    goal_section_state: GoalSectionState,
    sidebar_state: ProofStepsSidebarState,
    filters: HypothesisFilters,
    selection: SelectionState,
    /// Which pane is currently focused for keyboard navigation.
    focused_pane: FocusedPane,
    /// Last known sidebar viewport height (for scroll bounds).
    sidebar_viewport_height: usize,
}

impl StepsMode {
    fn selectable_items(&self) -> Vec<Selection> {
        let Some(node_id) = self.current_node_id else {
            return Vec::new();
        };

        // All hypotheses first (matching the Hypotheses pane order),
        // then all goals (matching the Goals pane order).
        let hyps = self.goals.iter().flat_map(|goal| {
            hypothesis_indices(goal.hyps.len(), self.filters.reverse_order)
                .filter(|&i| self.filters.should_show(&goal.hyps[i]))
                .map(move |hyp_idx| Selection::Hyp { node_id, hyp_idx })
        });
        let goals = self
            .goals
            .iter()
            .enumerate()
            .map(move |(goal_idx, _)| Selection::Goal { node_id, goal_idx });

        hyps.chain(goals).collect()
    }

    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

    fn has_steps(&self) -> bool {
        self.proof_dag.as_ref().is_some_and(|dag| !dag.is_empty())
    }

    fn layout_with_sidebar(&self, area: Rect) -> (Rect, Option<Rect>) {
        if self.has_steps() {
            let [sidebar, main] = Layout::horizontal([
                Constraint::Ratio(3, 8), // Sidebar takes 3/8 of width
                Constraint::Ratio(5, 8), // Main content takes 5/8
            ])
            .areas(area);
            (main, Some(sidebar))
        } else {
            (area, None)
        }
    }

    fn layout_main(area: Rect) -> MainLayout {
        let [hyps, div, goals] = Layout::vertical([
            Constraint::Fill(1),   // Hypotheses expand
            Constraint::Length(1), // Divider is always 1 line
            Constraint::Fill(1),   // Goals expand equally
        ])
        .areas(area);
        MainLayout { hyps, div, goals }
    }

    fn current_node(&self) -> Option<&ProofDagNode> {
        self.proof_dag
            .as_ref()
            .and_then(|dag| dag.current_node.and_then(|id| dag.get(id)))
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.handle_focus_switch(&key) {
            return true;
        }
        if self.handle_filter_toggle(key.code) {
            return true;
        }
        self.handle_navigation(key.code)
    }

    fn handle_focus_switch(&mut self, key: &KeyEvent) -> bool {
        let switched = match key.code {
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.focused_pane = self.focused_pane.prev();
                true
            }
            KeyCode::Tab => {
                self.focused_pane = self.focused_pane.next();
                true
            }
            KeyCode::BackTab => {
                self.focused_pane = self.focused_pane.prev();
                true
            }
            _ => false,
        };
        if switched && self.focused_pane != FocusedPane::Sidebar {
            self.sidebar_state.reset_scroll();
        }
        switched
    }

    fn handle_filter_toggle(&mut self, code: KeyCode) -> bool {
        // Note: 'l' is only a filter toggle when NOT focused on Sidebar
        // (where it's used for horizontal scrolling)
        match code {
            KeyCode::Char('i') => {
                self.filters.toggle(FilterToggle::Instances);
                true
            }
            KeyCode::Char('a') => {
                self.filters.toggle(FilterToggle::Inaccessible);
                true
            }
            KeyCode::Char('l') if self.focused_pane != FocusedPane::Sidebar => {
                self.filters.toggle(FilterToggle::LetValues);
                true
            }
            KeyCode::Char('r') => {
                self.filters.toggle(FilterToggle::ReverseOrder);
                true
            }
            _ => false,
        }
    }

    fn handle_navigation(&mut self, code: KeyCode) -> bool {
        match self.focused_pane {
            FocusedPane::Sidebar => self.handle_sidebar_navigation(code),
            FocusedPane::Hypotheses | FocusedPane::Goals => self.handle_selection_navigation(code),
        }
    }

    fn handle_sidebar_navigation(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.sidebar_state.scroll_down(self.sidebar_viewport_height);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.sidebar_state.scroll_up();
                true
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.sidebar_state.scroll_left();
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.sidebar_state.scroll_right();
                true
            }
            _ => false,
        }
    }

    fn handle_selection_navigation(&mut self, code: KeyCode) -> bool {
        let items = self.selectable_items();
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.selection.select_next(items.len());
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selection.select_previous(items.len());
                true
            }
            _ => false,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        let items = self.selectable_items();
        mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && self.selection.handle_click(mouse.column, mouse.row, &items)
    }
}

struct MainLayout {
    hyps: Rect,
    div: Rect,
    goals: Rect,
}

impl InteractiveComponent for StepsMode {
    type Input = StepsModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        let goals_changed = self.goals != input.goals;
        self.goals = input.goals;
        self.definition = input.definition;
        self.error = input.error;
        self.current_node_id = input.proof_dag.as_ref().and_then(|dag| dag.current_node);
        self.proof_dag = input.proof_dag;

        if goals_changed {
            self.selection.reset(self.selectable_items().len());
        }
    }

    fn handle_event(&mut self, event: Self::Event) -> bool {
        match event {
            KeyMouseEvent::Key(key) => self.handle_key(key),
            KeyMouseEvent::Mouse(mouse) => self.handle_mouse(mouse),
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
            ProofStepsSidebar::update_state(&mut self.sidebar_state, self.proof_dag.clone());
            self.sidebar_state
                .set_focused(self.focused_pane == FocusedPane::Sidebar);
            // Store viewport height for scroll bounds
            self.sidebar_viewport_height = sidebar_area.height.saturating_sub(2) as usize;
            frame.render_stateful_widget(ProofStepsSidebar, sidebar_area, &mut self.sidebar_state);
        }

        // Layout main area
        let layout = Self::layout_main(main);

        // Collect context data from current node
        let current_node = self.current_node();

        let depends_on: HashSet<String> = current_node
            .map(|node| node.tactic.depends_on.iter().cloned().collect())
            .unwrap_or_default();

        let spawned_goal_ids: HashSet<String> = current_node
            .map(|node| {
                node.state_after
                    .goals
                    .iter()
                    .filter_map(|g| g.username.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let active_goal_name = current_node
            .and_then(|node| node.state_before.goals.first())
            .and_then(|g| g.username.as_str().map(String::from));

        // Render hypothesis section
        self.hyp_section_state.update(
            &self.goals,
            self.filters,
            depends_on,
            self.current_selection(),
            self.current_node_id,
        );
        self.hyp_section_state
            .set_focused(self.focused_pane == FocusedPane::Hypotheses);
        frame.render_stateful_widget(HypSection, layout.hyps, &mut self.hyp_section_state);
        for region in self.hyp_section_state.click_regions() {
            self.selection.add_region(region.area, region.selection);
        }

        // Render divider
        frame.render_widget(divider(), layout.div);

        // Render goal section
        self.goal_section_state.update(
            self.goals.clone(),
            self.current_selection(),
            spawned_goal_ids,
            self.current_node_id,
            active_goal_name,
        );
        self.goal_section_state
            .set_focused(self.focused_pane == FocusedPane::Goals);
        frame.render_stateful_widget(GoalSection, layout.goals, &mut self.goal_section_state);
        for region in self.goal_section_state.click_regions() {
            self.selection.add_region(region.area, region.selection);
        }
    }
}

impl Mode for StepsMode {
    type Model = StepsModeInput;

    const NAME: &'static str = "Steps";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[
        ("Tab", "pane"),
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
