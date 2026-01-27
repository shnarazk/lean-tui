//! Deduction Tree mode - Paperproof tree visualization.

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use super::{Backend, Mode};
use crate::{
    lean_rpc::Goal,
    tui::widgets::{
        render_helpers::{render_error, render_no_goals},
        tree_view::render_tree_view_from_dag,
        ClickRegion, FilterToggle, HypothesisFilters, InteractiveComponent, KeyMouseEvent,
        Selection,
    },
    tui_ipc::{DefinitionInfo, ProofDag},
};

/// Input for updating the Deduction Tree mode.
pub struct DeductionTreeModeInput {
    pub goals: Vec<Goal>,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub proof_dag: Option<ProofDag>,
}

/// Deduction Tree display mode - Paperproof tree visualization.
pub struct DeductionTreeMode {
    goals: Vec<Goal>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    proof_dag: Option<ProofDag>,
    filters: HypothesisFilters,
    /// Flat index into `tree_selectable_items()`.
    selected_idx: Option<usize>,
    /// When true, tree renders top-down (parent above children).
    /// When false, renders bottom-up (children above parent).
    tree_top_down: bool,
    /// Click regions from last render.
    click_regions: Vec<ClickRegion>,
}

impl Default for DeductionTreeMode {
    fn default() -> Self {
        Self {
            goals: Vec::new(),
            definition: None,
            error: None,
            proof_dag: None,
            filters: HypothesisFilters::default(),
            selected_idx: None,
            tree_top_down: true,
            click_regions: Vec::new(),
        }
    }
}

impl DeductionTreeMode {
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

    fn select_next(&mut self) {
        let count = self.tree_selectable_items().len();
        if count == 0 {
            return;
        }
        self.selected_idx = Some(self.selected_idx.map_or(0, |idx| (idx + 1) % count));
    }

    fn select_previous(&mut self) {
        let count = self.tree_selectable_items().len();
        if count == 0 {
            return;
        }
        self.selected_idx = Some(
            self.selected_idx
                .map_or(count - 1, |idx| idx.checked_sub(1).unwrap_or(count - 1)),
        );
    }

    fn select_by_selection(&mut self, sel: Selection) {
        let items = self.tree_selectable_items();
        if let Some(idx) = items.iter().position(|s| *s == sel) {
            self.selected_idx = Some(idx);
        }
    }

    fn find_click_at(&self, x: u16, y: u16) -> Option<Selection> {
        self.click_regions
            .iter()
            .find(|r| {
                x >= r.area.x
                    && x < r.area.x + r.area.width
                    && y >= r.area.y
                    && y < r.area.y + r.area.height
            })
            .map(|r| r.selection)
    }
}

impl InteractiveComponent for DeductionTreeMode {
    type Input = DeductionTreeModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        // Reset selection when step count changes
        let old_count = self.proof_dag.as_ref().map_or(0, ProofDag::len);
        let new_count = input.proof_dag.as_ref().map_or(0, ProofDag::len);

        self.goals = input.goals;
        self.definition = input.definition;
        self.error = input.error;
        self.proof_dag = input.proof_dag;

        if old_count != new_count {
            self.selected_idx = None;
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
                KeyCode::Char('r') => {
                    self.tree_top_down = !self.tree_top_down;
                    true
                }
                _ => false,
            },
            KeyMouseEvent::Mouse(mouse) => {
                let is_click = mouse.kind == MouseEventKind::Down(MouseButton::Left);
                let clicked_item = is_click
                    .then(|| self.find_click_at(mouse.column, mouse.row))
                    .flatten();
                if let Some(sel) = clicked_item {
                    self.select_by_selection(sel);
                    return true;
                }
                false
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let content_area = render_error(frame, area, self.error.as_deref());

        if self.goals.is_empty() {
            render_no_goals(frame, content_area);
            return;
        }

        // Render tree view using proof_dag
        if let Some(ref dag) = self.proof_dag {
            render_tree_view_from_dag(
                frame,
                content_area,
                dag,
                self.tree_top_down,
                self.current_tree_selection(),
                &mut self.click_regions,
            );
        } else {
            frame.render_widget(
                Paragraph::new("No proof steps available").style(Style::new().fg(Color::DarkGray)),
                content_area,
            );
        }
    }
}

impl Mode for DeductionTreeMode {
    type Model = DeductionTreeModeInput;

    const NAME: &'static str = "Deduction Tree";
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[("r", "dir")];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[];
    const BACKENDS: &'static [Backend] = &[Backend::Paperproof, Backend::TreeSitter];

    fn current_selection(&self) -> Option<Selection> {
        // Return tree-specific selection directly - app.rs handles
        // InitialHyp, StepHyp, StepGoal, Theorem by looking up in ProofDag
        self.current_tree_selection()
    }
}
