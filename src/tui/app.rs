//! Application state and event handling.

use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::{layout::Rect, widgets::ListState};

use crate::{
    lean_rpc::{Goal, Hypothesis},
    tui_ipc::{Command, CursorInfo, GoalResult, Message, Position, TemporalSlot},
};

/// A selectable item in the TUI (hypothesis or goal target).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectableItem {
    Hypothesis { goal_idx: usize, hyp_idx: usize },
    GoalTarget { goal_idx: usize },
}

/// A clickable region mapped to a selectable item.
#[derive(Debug, Clone)]
pub struct ClickRegion {
    pub area: Rect,
    pub item: SelectableItem,
}

/// Visibility settings for diff columns (both hidden by default).
#[derive(Debug, Clone, Copy, Default)]
pub struct ColumnVisibility {
    /// Show the "Previous" column in diff view.
    pub previous: bool,
    /// Show the "Next" column in diff view.
    pub next: bool,
}

/// Loading status for goal state.
#[derive(Debug, Clone, Default)]
pub enum LoadStatus {
    #[default]
    Ready,
    Loading,
    NotAvailable,
    Error(String),
}

/// Goal state at a specific position.
#[derive(Debug, Clone, Default)]
pub struct GoalState {
    pub goals: Vec<Goal>,
    pub position: Position,
    pub status: LoadStatus,
}

/// Goals at three temporal positions (previous, current, next tactic).
#[derive(Debug, Clone, Default)]
pub struct TemporalGoals {
    /// Goals before the last tactic.
    pub previous: Option<GoalState>,
    /// Goals at current cursor position.
    pub current: GoalState,
    /// Goals after the next tactic.
    pub next: Option<GoalState>,
}

/// Filter settings for hypothesis display (VSCode-lean4 compatible).
#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct HypothesisFilters {
    /// Hide typeclass instance hypotheses.
    pub hide_instances: bool,
    /// Hide type hypotheses.
    pub hide_types: bool,
    /// Hide inaccessible names (names containing dagger U+2020).
    pub hide_inaccessible: bool,
    /// Hide let-binding values (show `x : T` instead of `x : T := v`).
    pub hide_let_values: bool,
    /// Reverse hypothesis order (newest first).
    pub reverse_order: bool,
}

impl HypothesisFilters {
    /// Check if a hypothesis should be shown based on current filters.
    pub fn should_show(self, hyp: &Hypothesis) -> bool {
        if self.hide_instances && hyp.is_instance {
            return false;
        }
        if self.hide_types && hyp.is_type {
            return false;
        }
        if self.hide_inaccessible && hyp.names.iter().any(|n| n.contains('\u{2020}')) {
            return false;
        }
        true
    }
}

/// Application state.
#[derive(Default)]
pub struct App {
    /// Current cursor position from editor.
    pub cursor: CursorInfo,
    /// Goals at three temporal positions.
    pub temporal_goals: TemporalGoals,
    /// Current error message.
    pub error: Option<String>,
    /// Whether connected to proxy.
    pub connected: bool,
    /// Selection state for the goal list.
    pub list_state: ListState,
    /// Whether app should exit.
    pub should_exit: bool,
    /// Pending navigation command to send.
    pub pending_navigation: Option<Command>,
    /// Pending commands to send (for temporal goal fetching).
    pub pending_commands: Vec<Command>,
    /// Click regions for mouse interaction (updated each render).
    pub click_regions: Vec<ClickRegion>,
    /// Visibility settings for diff columns.
    pub columns: ColumnVisibility,
    /// Filter settings for hypothesis display.
    pub filters: HypothesisFilters,
    /// Whether to show the help popup.
    pub show_help: bool,
}

/// Convert a goal result to a goal state.
fn goal_state_from_result(result: GoalResult, cursor_position: Position) -> GoalState {
    match result {
        GoalResult::Ready { position, goals } => GoalState {
            goals,
            position,
            status: LoadStatus::Ready,
        },
        GoalResult::NotAvailable => GoalState {
            goals: vec![],
            position: cursor_position,
            status: LoadStatus::NotAvailable,
        },
        GoalResult::Error { error } => GoalState {
            goals: vec![],
            position: cursor_position,
            status: LoadStatus::Error(error),
        },
    }
}

impl App {
    /// Get current goals (convenience accessor).
    pub fn goals(&self) -> &[Goal] {
        &self.temporal_goals.current.goals
    }

    /// Get position where current goals were fetched.
    pub const fn goals_position(&self) -> Option<Position> {
        if self.temporal_goals.current.goals.is_empty() {
            None
        } else {
            Some(self.temporal_goals.current.position)
        }
    }
}

impl App {
    /// Get all selectable items as a flat list, respecting current filters.
    pub fn selectable_items(&self) -> Vec<SelectableItem> {
        let mut items = Vec::new();
        for (goal_idx, goal) in self.goals().iter().enumerate() {
            // Build hypothesis indices, respecting reverse order
            let hyp_indices: Vec<usize> = if self.filters.reverse_order {
                (0..goal.hyps.len()).rev().collect()
            } else {
                (0..goal.hyps.len()).collect()
            };

            // Only include hypotheses that pass the filter
            items.extend(
                hyp_indices
                    .into_iter()
                    .filter(|&hyp_idx| self.filters.should_show(&goal.hyps[hyp_idx]))
                    .map(|hyp_idx| SelectableItem::Hypothesis { goal_idx, hyp_idx }),
            );
            items.push(SelectableItem::GoalTarget { goal_idx });
        }
        items
    }

    /// Get currently selected item.
    pub fn current_selection(&self) -> Option<SelectableItem> {
        self.list_state
            .selected()
            .and_then(|i| self.selectable_items().get(i).cloned())
    }

    /// Move selection to previous item.
    pub fn select_previous(&mut self) {
        let count = self.selectable_items().len();
        if count == 0 {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map_or(0, |i| i.saturating_sub(1));
        self.list_state.select(Some(i));
    }

    /// Move selection to next item.
    pub fn select_next(&mut self) {
        let count = self.selectable_items().len();
        if count == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) if i < count - 1 => i + 1,
            Some(i) => i,
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// Handle incoming message from proxy.
    pub fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Connected => {
                self.connected = true;
            }
            Message::Cursor(cursor) => {
                self.cursor = cursor;
                self.connected = true;
                self.error = None;
            }
            Message::Goals {
                uri: _,
                position,
                goals,
            } => {
                let goals_changed = self.temporal_goals.current.goals != goals;
                let line_changed = self.temporal_goals.current.position.line != position.line;

                // Skip temporal refresh if goals and line are unchanged
                // (character position changes on same line don't affect temporal context)
                if !goals_changed && !line_changed {
                    self.temporal_goals.current.position = position;
                    self.connected = true;
                    return;
                }

                self.temporal_goals.current = GoalState {
                    goals,
                    position,
                    status: LoadStatus::Ready,
                };
                self.connected = true;
                self.error = None;
                self.reset_selection();
                // Re-request temporal goals if columns are visible
                self.refresh_temporal_columns();
            }
            Message::TemporalGoals {
                uri: _,
                cursor_position,
                slot,
                result,
            } => {
                self.connected = true;
                // Only apply if still at same cursor position
                if cursor_position != self.cursor.position {
                    return;
                }
                let goal_state = goal_state_from_result(result, cursor_position);
                self.apply_temporal_goal(slot, goal_state);
            }
            Message::Error { error } => {
                self.error = Some(error);
                self.connected = true;
            }
        }
    }

    /// Handle terminal event. Returns true if event was handled.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return false;
                }
                match key.code {
                    KeyCode::Char('q') => {
                        self.should_exit = true;
                        true
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.select_next();
                        true
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.select_previous();
                        true
                    }
                    KeyCode::Enter => {
                        self.navigate_to_selection();
                        true
                    }
                    KeyCode::Char('p') => {
                        self.toggle_previous_column();
                        true
                    }
                    KeyCode::Char('n') => {
                        self.toggle_next_column();
                        true
                    }
                    // Hypothesis filter toggles
                    KeyCode::Char('i') => {
                        self.filters.hide_instances = !self.filters.hide_instances;
                        true
                    }
                    KeyCode::Char('t') => {
                        self.filters.hide_types = !self.filters.hide_types;
                        true
                    }
                    KeyCode::Char('a') => {
                        self.filters.hide_inaccessible = !self.filters.hide_inaccessible;
                        true
                    }
                    KeyCode::Char('l') => {
                        self.filters.hide_let_values = !self.filters.hide_let_values;
                        true
                    }
                    KeyCode::Char('r') => {
                        self.filters.reverse_order = !self.filters.reverse_order;
                        true
                    }
                    KeyCode::Char('?') => {
                        self.show_help = !self.show_help;
                        true
                    }
                    KeyCode::Esc if self.show_help => {
                        self.show_help = false;
                        true
                    }
                    _ => false,
                }
            }
            Event::Mouse(mouse) => {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    self.handle_click(mouse.column, mouse.row)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Handle mouse click at given coordinates.
    fn handle_click(&mut self, x: u16, y: u16) -> bool {
        let Some(region) = self.find_click_region(x, y) else {
            return false;
        };

        let items = self.selectable_items();
        let Some(idx) = items.iter().position(|item| *item == region.item) else {
            return false;
        };

        self.list_state.select(Some(idx));
        self.navigate_to_selection();
        true
    }

    /// Find the click region containing the given coordinates.
    fn find_click_region(&self, x: u16, y: u16) -> Option<&ClickRegion> {
        self.click_regions.iter().find(|region| {
            region.area.x <= x
                && x < region.area.x + region.area.width
                && region.area.y <= y
                && y < region.area.y + region.area.height
        })
    }

    /// Re-request temporal goals for visible columns.
    fn refresh_temporal_columns(&mut self) {
        if self.columns.previous {
            self.request_temporal_goals(TemporalSlot::Previous);
        }
        if self.columns.next {
            self.request_temporal_goals(TemporalSlot::Next);
        }
    }

    /// Toggle the previous column visibility.
    fn toggle_previous_column(&mut self) {
        self.columns.previous = !self.columns.previous;
        if self.columns.previous && self.temporal_goals.previous.is_none() {
            self.request_temporal_goals(TemporalSlot::Previous);
        }
    }

    /// Toggle the next column visibility.
    fn toggle_next_column(&mut self) {
        self.columns.next = !self.columns.next;
        if self.columns.next && self.temporal_goals.next.is_none() {
            self.request_temporal_goals(TemporalSlot::Next);
        }
    }

    /// Request temporal goals for a slot.
    fn request_temporal_goals(&mut self, slot: TemporalSlot) {
        let uri = self.cursor.uri.clone();
        if uri.is_empty() {
            return;
        }

        // Only set loading state if we don't have existing data
        // This prevents the "flash" when refreshing
        match slot {
            TemporalSlot::Previous => {
                if self.temporal_goals.previous.is_none() {
                    self.temporal_goals.previous = Some(GoalState {
                        goals: vec![],
                        position: self.cursor.position,
                        status: LoadStatus::Loading,
                    });
                }
            }
            TemporalSlot::Next => {
                if self.temporal_goals.next.is_none() {
                    self.temporal_goals.next = Some(GoalState {
                        goals: vec![],
                        position: self.cursor.position,
                        status: LoadStatus::Loading,
                    });
                }
            }
            TemporalSlot::Current => {}
        }

        self.pending_commands.push(Command::FetchTemporalGoals {
            uri,
            cursor_position: self.cursor.position,
            slot,
        });
    }

    /// Apply a temporal goal state to the appropriate slot.
    fn apply_temporal_goal(&mut self, slot: TemporalSlot, goal_state: GoalState) {
        match slot {
            TemporalSlot::Previous => self.temporal_goals.previous = Some(goal_state),
            TemporalSlot::Current => {
                self.temporal_goals.current = goal_state;
                self.reset_selection();
            }
            TemporalSlot::Next => self.temporal_goals.next = Some(goal_state),
        }
    }

    /// Reset selection state based on available items.
    fn reset_selection(&mut self) {
        if self.selectable_items().is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    /// Navigate to the currently selected item.
    ///
    /// For hypotheses with `info` (from `SubexprInfo`), sends a
    /// `GetHypothesisLocation` command to look up the actual definition
    /// location via `getGoToLocation` RPC. Falls back to `goals_position`
    /// if no info is available.
    fn navigate_to_selection(&mut self) {
        let Some(selection) = self.current_selection() else {
            return;
        };

        // Get the URI from cursor info
        let uri = self.cursor.uri.clone();
        if uri.is_empty() {
            return;
        }

        // Get the position where goals were fetched (for RPC session context)
        let goals_pos = self.goals_position().unwrap_or(self.cursor.position);

        self.pending_navigation = Some(self.build_navigation_command(&selection, uri, goals_pos));
    }

    /// Build the appropriate navigation command for the selected item.
    fn build_navigation_command(
        &self,
        selection: &SelectableItem,
        uri: String,
        goals_pos: Position,
    ) -> Command {
        // Try to get hypothesis info for go-to-definition
        let hyp_info = match selection {
            SelectableItem::Hypothesis { goal_idx, hyp_idx } => self
                .goals()
                .get(*goal_idx)
                .and_then(|g| g.hyps.get(*hyp_idx))
                .and_then(|h| h.info.clone()),
            SelectableItem::GoalTarget { .. } => None,
        };

        if let Some(info) = hyp_info {
            // We have info - request location lookup from proxy
            Command::GetHypothesisLocation {
                uri,
                line: goals_pos.line,
                character: goals_pos.character,
                info,
            }
        } else {
            // Fallback: navigate to goals position
            Command::Navigate {
                uri,
                line: goals_pos.line,
                character: goals_pos.character,
            }
        }
    }

    /// Take the pending navigation command, if any.
    #[allow(clippy::missing_const_for_fn)] // Option::take is not const-stable
    pub fn take_pending_navigation(&mut self) -> Option<Command> {
        self.pending_navigation.take()
    }

    /// Take all pending commands (for temporal goal fetching).
    pub fn take_pending_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.pending_commands)
    }
}
