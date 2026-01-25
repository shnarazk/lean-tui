//! Application state and event handling.

use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::{layout::Rect, widgets::ListState};

use crate::{
    lean_rpc::Goal,
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
    /// Pending command to send (for temporal goal fetching).
    pub pending_command: Option<Command>,
    /// Click regions for mouse interaction (updated each render).
    pub click_regions: Vec<ClickRegion>,
    /// Visibility settings for diff columns.
    pub columns: ColumnVisibility,
}

impl App {
    /// Get current goals (convenience accessor).
    pub fn goals(&self) -> &[Goal] {
        &self.temporal_goals.current.goals
    }

    /// Get position where current goals were fetched.
    pub fn goals_position(&self) -> Option<Position> {
        if self.temporal_goals.current.goals.is_empty() {
            None
        } else {
            Some(self.temporal_goals.current.position)
        }
    }
}

impl App {
    /// Get all selectable items as a flat list.
    pub fn selectable_items(&self) -> Vec<SelectableItem> {
        let mut items = Vec::new();
        for (goal_idx, goal) in self.goals().iter().enumerate() {
            for hyp_idx in 0..goal.hyps.len() {
                items.push(SelectableItem::Hypothesis { goal_idx, hyp_idx });
            }
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
            Message::Cursor(cursor) => {
                // Clear temporal goals when cursor moves (they'll be re-fetched)
                if cursor.position != self.cursor.position || cursor.uri != self.cursor.uri {
                    self.temporal_goals.previous = None;
                    self.temporal_goals.next = None;
                }
                self.cursor = cursor;
                self.connected = true;
                self.error = None;
            }
            Message::Goals {
                uri: _,
                position,
                goals,
            } => {
                // Legacy Goals message - treat as current slot
                self.temporal_goals.current = GoalState {
                    goals,
                    position,
                    status: LoadStatus::Ready,
                };
                // Clear previous/next when current changes
                self.temporal_goals.previous = None;
                self.temporal_goals.next = None;
                self.connected = true;
                self.error = None;
                // Reset selection when goals change
                if self.selectable_items().is_empty() {
                    self.list_state.select(None);
                } else {
                    self.list_state.select(Some(0));
                }
            }
            Message::TemporalGoals {
                uri: _,
                cursor_position,
                slot,
                result,
            } => {
                // Only apply if still at same cursor position
                if cursor_position == self.cursor.position {
                    let goal_state = match result {
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
                    };

                    match slot {
                        TemporalSlot::Previous => {
                            self.temporal_goals.previous = Some(goal_state);
                        }
                        TemporalSlot::Current => {
                            self.temporal_goals.current = goal_state;
                            // Reset selection when current goals change
                            if self.selectable_items().is_empty() {
                                self.list_state.select(None);
                            } else {
                                self.list_state.select(Some(0));
                            }
                        }
                        TemporalSlot::Next => {
                            self.temporal_goals.next = Some(goal_state);
                        }
                    }
                }
                self.connected = true;
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
                        self.columns.previous = !self.columns.previous;
                        // Request previous goals if column toggled on and not already loaded
                        if self.columns.previous && self.temporal_goals.previous.is_none() {
                            self.request_temporal_goals(TemporalSlot::Previous);
                        }
                        true
                    }
                    KeyCode::Char('n') => {
                        self.columns.next = !self.columns.next;
                        // Request next goals if column toggled on and not already loaded
                        if self.columns.next && self.temporal_goals.next.is_none() {
                            self.request_temporal_goals(TemporalSlot::Next);
                        }
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

    /// Request temporal goals for a slot.
    fn request_temporal_goals(&mut self, slot: TemporalSlot) {
        let uri = self.cursor.uri.clone();
        if uri.is_empty() {
            return;
        }

        // Mark as loading
        match slot {
            TemporalSlot::Previous => {
                self.temporal_goals.previous = Some(GoalState {
                    goals: vec![],
                    position: self.cursor.position,
                    status: LoadStatus::Loading,
                });
            }
            TemporalSlot::Next => {
                self.temporal_goals.next = Some(GoalState {
                    goals: vec![],
                    position: self.cursor.position,
                    status: LoadStatus::Loading,
                });
            }
            TemporalSlot::Current => {}
        }

        self.pending_command = Some(Command::FetchTemporalGoals {
            uri,
            cursor_position: self.cursor.position,
            slot,
        });
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

    /// Take the pending command, if any (for temporal goal fetching).
    #[allow(clippy::missing_const_for_fn)]
    pub fn take_pending_command(&mut self) -> Option<Command> {
        self.pending_command.take()
    }
}
