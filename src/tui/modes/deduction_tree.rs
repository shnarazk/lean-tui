//! Deduction Tree mode - Paperproof tree visualization.

use std::collections::HashSet;

use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

/// A click region mapping screen area to a tree selection.
#[derive(Debug, Clone)]
pub struct TreeClickRegion {
    pub area: Rect,
    pub selection: TreeSelection,
}

/// Collects click regions during rendering.
#[derive(Default)]
pub struct TreeClickRegions {
    pub regions: Vec<TreeClickRegion>,
}

impl TreeClickRegions {
    pub fn add(&mut self, area: Rect, selection: TreeSelection) {
        self.regions.push(TreeClickRegion { area, selection });
    }

    pub fn find_at(&self, x: u16, y: u16) -> Option<TreeSelection> {
        self.regions
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

use super::{Backend, Mode};
use crate::{
    lean_rpc::{Goal, PaperproofStep},
    tui::widgets::{
        interactive_widget::InteractiveWidget,
        render_helpers::{render_error, render_no_goals},
        tree_view::render_tree_view,
        FilterToggle, HypothesisFilters, KeyMouseEvent, SelectableItem,
    },
    tui_ipc::DefinitionInfo,
};

/// Selection within the deduction tree.
/// Tracks step index and item index within that step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeSelection {
    /// Initial hypothesis (from `goal_before` of first step).
    InitialHyp { hyp_idx: usize },
    /// Hypothesis introduced by a step.
    StepHyp { step_idx: usize, hyp_idx: usize },
    /// Goal after a step.
    StepGoal { step_idx: usize, goal_idx: usize },
    /// The theorem (final goal).
    Theorem,
}

/// Input for updating the Deduction Tree mode.
pub struct DeductionTreeModeInput {
    pub goals: Vec<Goal>,
    pub definition: Option<DefinitionInfo>,
    pub error: Option<String>,
    pub current_step_index: usize,
    pub paperproof_steps: Option<Vec<PaperproofStep>>,
}

/// Deduction Tree display mode - Paperproof tree visualization.
pub struct DeductionTreeMode {
    goals: Vec<Goal>,
    definition: Option<DefinitionInfo>,
    error: Option<String>,
    current_step_index: usize,
    paperproof_steps: Option<Vec<PaperproofStep>>,
    filters: HypothesisFilters,
    /// Flat index into `tree_selectable_items()`.
    selected_idx: Option<usize>,
    /// When true, tree renders top-down (parent above children).
    /// When false, renders bottom-up (children above parent).
    tree_top_down: bool,
    /// Click regions from last render.
    click_regions: TreeClickRegions,
}

impl Default for DeductionTreeMode {
    fn default() -> Self {
        Self {
            goals: Vec::new(),
            definition: None,
            error: None,
            current_step_index: 0,
            paperproof_steps: None,
            filters: HypothesisFilters::default(),
            selected_idx: None,
            tree_top_down: true,
            click_regions: TreeClickRegions::default(),
        }
    }
}

impl DeductionTreeMode {
    pub const fn filters(&self) -> HypothesisFilters {
        self.filters
    }

    /// Build list of all selectable items in the tree.
    fn tree_selectable_items(&self) -> Vec<TreeSelection> {
        let Some(steps) = &self.paperproof_steps else {
            return Vec::new();
        };
        if steps.is_empty() {
            return Vec::new();
        }

        let mut items = Vec::new();

        // Initial hypotheses (non-proof ones from first step's goal_before)
        for (idx, h) in steps[0].goal_before.hyps.iter().enumerate() {
            if h.is_proof != "proof" {
                items.push(TreeSelection::InitialHyp { hyp_idx: idx });
            }
        }

        // For each step: new hypotheses and goals
        for (step_idx, step) in steps.iter().enumerate() {
            // New hypotheses introduced by this step
            let before_ids: HashSet<&str> = step
                .goal_before
                .hyps
                .iter()
                .map(|h| h.id.as_str())
                .collect();

            let new_hyp_items = step.goals_after.first().into_iter().flat_map(|g| {
                g.hyps
                    .iter()
                    .enumerate()
                    .filter(|(_, h)| !before_ids.contains(h.id.as_str()))
                    .map(move |(hyp_idx, _)| TreeSelection::StepHyp { step_idx, hyp_idx })
            });
            items.extend(new_hyp_items);

            // Goals after this step
            let goal_items = step
                .goals_after
                .iter()
                .enumerate()
                .map(|(goal_idx, _)| TreeSelection::StepGoal { step_idx, goal_idx });
            items.extend(goal_items);
        }

        // Theorem (final goal)
        items.push(TreeSelection::Theorem);

        items
    }

    fn current_tree_selection(&self) -> Option<TreeSelection> {
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

    fn select_by_tree_selection(&mut self, sel: TreeSelection) {
        let items = self.tree_selectable_items();
        if let Some(idx) = items.iter().position(|s| *s == sel) {
            self.selected_idx = Some(idx);
        }
    }
}

impl InteractiveWidget for DeductionTreeMode {
    type Input = DeductionTreeModeInput;
    type Event = KeyMouseEvent;

    fn update(&mut self, input: Self::Input) {
        // Reset selection when step count changes
        let old_count = self.paperproof_steps.as_ref().map_or(0, Vec::len);
        let new_count = input.paperproof_steps.as_ref().map_or(0, Vec::len);

        self.goals = input.goals;
        self.definition = input.definition;
        self.error = input.error;
        self.current_step_index = input.current_step_index;
        self.paperproof_steps = input.paperproof_steps;

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
                    .then(|| self.click_regions.find_at(mouse.column, mouse.row))
                    .flatten();
                if let Some(sel) = clicked_item {
                    self.select_by_tree_selection(sel);
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

        // Render tree view
        if let Some(ref steps) = self.paperproof_steps {
            render_tree_view(
                frame,
                content_area,
                steps,
                self.current_step_index,
                self.tree_top_down,
                self.current_tree_selection(),
                &mut self.click_regions,
            );
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
    const KEYBINDINGS: &'static [(&'static str, &'static str)] = &[("r", "dir")];
    const SUPPORTED_FILTERS: &'static [FilterToggle] = &[];
    const BACKENDS: &'static [Backend] = &[Backend::Paperproof];

    fn current_selection(&self) -> Option<SelectableItem> {
        // Tree uses its own selection system (TreeSelection)
        None
    }
}
