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

    /// Find the nearest selectable item in the given direction using grid-based navigation.
    fn find_nearest_in_direction(&self, direction: Direction) -> Option<Selection> {
        let current_sel = self.current_tree_selection()?;
        let cur = self
            .click_regions
            .iter()
            .find(|r| r.selection == current_sel)?
            .area;

        let is_horizontal = matches!(direction, Direction::Left | Direction::Right);

        // Grid-aligned search: find items that overlap on the perpendicular axis
        let aligned = self.click_regions.iter().filter(|r| {
            let dominated = if is_horizontal {
                ranges_overlap(cur.y, cur.height, r.area.y, r.area.height)
            } else {
                ranges_overlap(cur.x, cur.width, r.area.x, r.area.width)
            };
            dominated && direction.is_ahead(cur, r.area)
        });

        // Fallback: any item in the direction
        let fallback = self
            .click_regions
            .iter()
            .filter(|r| direction.is_ahead(cur, r.area));

        aligned
            .chain(fallback)
            .min_by_key(|r| direction.distance(cur, r.area))
            .map(|r| r.selection)
    }

    /// Get the active goal selection (first goal of the current node).
    fn active_goal_selection(&self) -> Option<Selection> {
        let dag = self.proof_dag.as_ref()?;
        let current_node_id = dag.current_node?;
        // Select the first goal of the current node
        Some(Selection::Goal {
            node_id: current_node_id,
            goal_idx: 0,
        })
    }

    fn move_in_direction(&mut self, direction: Direction) -> bool {
        // If nothing is selected, start at the active goal
        if self.selected_idx.is_none() {
            if let Some(sel) = self.active_goal_selection() {
                self.select_by_selection(sel);
                return true;
            }
        }

        self.find_nearest_in_direction(direction)
            .map(|sel| self.select_by_selection(sel))
            .is_some()
    }
}

/// Direction for spatial navigation.
#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// Check if `other` is ahead of `cur` in this direction.
    const fn is_ahead(self, cur: Rect, other: Rect) -> bool {
        match self {
            Self::Left => other.x + other.width <= cur.x,
            Self::Right => other.x >= cur.x + cur.width,
            Self::Up => other.y + other.height <= cur.y,
            Self::Down => other.y >= cur.y + cur.height,
        }
    }

    /// Distance from `cur` to `other` along this direction's axis.
    const fn distance(self, cur: Rect, other: Rect) -> u16 {
        match self {
            Self::Left => cur.x.saturating_sub(other.x + other.width),
            Self::Right => other.x.saturating_sub(cur.x + cur.width),
            Self::Up => cur.y.saturating_sub(other.y + other.height),
            Self::Down => other.y.saturating_sub(cur.y + cur.height),
        }
    }
}

/// Check if two 1D ranges overlap.
const fn ranges_overlap(a_start: u16, a_len: u16, b_start: u16, b_len: u16) -> bool {
    a_start < b_start + b_len && b_start < a_start + a_len
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
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[("hjkl", "nav"), ("t", "dir")];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[];
    const BACKENDS: &'static [Backend] = &[Backend::Paperproof, Backend::TreeSitter];

    fn current_selection(&self) -> Option<Selection> {
        // Return tree-specific selection directly - app.rs handles
        // InitialHyp, StepHyp, StepGoal, Theorem by looking up in ProofDag
        self.current_tree_selection()
    }
}
