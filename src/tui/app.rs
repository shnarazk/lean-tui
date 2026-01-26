//! Application state for the TUI.

use std::mem;

use async_lsp::lsp_types::Url;

use super::components::SelectableItem;
use crate::{
    lean_rpc::{Goal, GotoLocation},
    tui_ipc::{
        CaseSplitInfo, Command, CursorInfo, DefinitionInfo, GoalResult, Message, Position,
        TemporalSlot,
    },
};

/// Visibility settings for diff columns (both hidden by default).
#[derive(Debug, Clone, Copy, Default)]
pub struct ColumnVisibility {
    pub previous: bool,
    pub next: bool,
}

/// Loading status for goal state (used for temporal goals feature).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
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
    #[allow(dead_code)]
    pub status: LoadStatus,
}

/// Goals at three temporal positions (previous, current, next tactic).
#[derive(Debug, Clone, Default)]
pub struct TemporalGoals {
    pub previous: Option<GoalState>,
    pub current: GoalState,
    pub next: Option<GoalState>,
}

/// Application state.
#[derive(Default)]
pub struct App {
    /// Current cursor position from editor.
    pub cursor: Option<CursorInfo>,
    /// Goals at three temporal positions.
    pub temporal_goals: TemporalGoals,
    /// Enclosing definition (theorem/lemma name).
    pub definition: Option<DefinitionInfo>,
    /// Case-splitting tactics affecting current position.
    pub case_splits: Vec<CaseSplitInfo>,
    /// Current error message.
    pub error: Option<String>,
    /// Whether connected to proxy.
    pub connected: bool,
    /// Whether app should exit.
    pub should_exit: bool,
    /// Outgoing commands queue.
    outgoing_commands: Vec<Command>,
    /// Visibility settings for diff columns.
    pub columns: ColumnVisibility,
}

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
    /// Get current goals.
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

    /// Queue a command to be sent to the proxy.
    pub fn queue_command(&mut self, cmd: Command) {
        self.outgoing_commands.push(cmd);
    }

    /// Take all queued commands.
    pub fn take_commands(&mut self) -> Vec<Command> {
        mem::take(&mut self.outgoing_commands)
    }

    /// Handle incoming message from proxy.
    pub fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Connected => {
                self.connected = true;
            }
            Message::Cursor(cursor) => {
                self.cursor = Some(cursor);
                self.connected = true;
                self.error = None;
            }
            Message::Goals {
                uri: _,
                position,
                goals,
                definition,
                case_splits,
            } => {
                let goals_changed = self.temporal_goals.current.goals != goals;
                let line_changed = self.temporal_goals.current.position.line != position.line;

                // Keep previous definition if new one is None and we're on the same line
                let new_definition = if definition.is_none() && !line_changed {
                    self.definition.clone()
                } else {
                    definition
                };

                if !goals_changed && !line_changed {
                    self.temporal_goals.current.position = position;
                    self.definition = new_definition;
                    self.case_splits = case_splits;
                    self.connected = true;
                    return;
                }

                self.temporal_goals.current = GoalState {
                    goals,
                    position,
                    status: LoadStatus::Ready,
                };
                self.definition = new_definition;
                self.case_splits = case_splits;
                self.connected = true;
                self.error = None;
                self.refresh_temporal_columns();
            }
            Message::TemporalGoals {
                uri: _,
                cursor_position,
                slot,
                result,
            } => {
                self.connected = true;
                let dominated_position = self.cursor.as_ref().map(|c| c.position);
                if Some(cursor_position) != dominated_position {
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

    /// Toggle the previous column visibility.
    pub fn toggle_previous_column(&mut self) {
        self.columns.previous = !self.columns.previous;
        if self.columns.previous && self.temporal_goals.previous.is_none() {
            self.request_temporal_goals(TemporalSlot::Previous);
        }
    }

    /// Toggle the next column visibility.
    pub fn toggle_next_column(&mut self) {
        self.columns.next = !self.columns.next;
        if self.columns.next && self.temporal_goals.next.is_none() {
            self.request_temporal_goals(TemporalSlot::Next);
        }
    }

    /// Navigate to the given selection.
    pub fn navigate_to_selection(&mut self, selection: Option<SelectableItem>) {
        let Some(cursor) = &self.cursor else {
            return;
        };

        let goals_pos = self.goals_position().unwrap_or(cursor.position);
        let cmd = self.build_navigation_command(selection, cursor.uri.clone(), goals_pos);
        self.queue_command(cmd);
    }

    fn build_navigation_command(
        &self,
        selection: Option<SelectableItem>,
        uri: Url,
        position: Position,
    ) -> Command {
        let goto_location: Option<&GotoLocation> = match selection {
            Some(SelectableItem::Hypothesis { goal_idx, hyp_idx }) => self
                .goals()
                .get(goal_idx)
                .and_then(|g| g.hyps.get(hyp_idx))
                .and_then(|h| h.goto_location.as_ref()),
            Some(SelectableItem::GoalTarget { goal_idx }) => self
                .goals()
                .get(goal_idx)
                .and_then(|g| g.goto_location.as_ref()),
            None => None,
        };

        goto_location.map_or(Command::Navigate { uri, position }, |loc| {
            Command::Navigate {
                uri: loc.uri.clone(),
                position: loc.position,
            }
        })
    }

    fn refresh_temporal_columns(&mut self) {
        if self.columns.previous {
            self.request_temporal_goals(TemporalSlot::Previous);
        }
        if self.columns.next {
            self.request_temporal_goals(TemporalSlot::Next);
        }
    }

    fn request_temporal_goals(&mut self, slot: TemporalSlot) {
        let Some(cursor) = &self.cursor else {
            return;
        };

        match slot {
            TemporalSlot::Previous => {
                if self.temporal_goals.previous.is_none() {
                    self.temporal_goals.previous = Some(GoalState {
                        goals: vec![],
                        position: cursor.position,
                        status: LoadStatus::Loading,
                    });
                }
            }
            TemporalSlot::Next => {
                if self.temporal_goals.next.is_none() {
                    self.temporal_goals.next = Some(GoalState {
                        goals: vec![],
                        position: cursor.position,
                        status: LoadStatus::Loading,
                    });
                }
            }
            TemporalSlot::Current => {}
        }

        self.queue_command(Command::FetchTemporalGoals {
            uri: cursor.uri.clone(),
            cursor_position: cursor.position,
            slot,
        });
    }

    fn apply_temporal_goal(&mut self, slot: TemporalSlot, goal_state: GoalState) {
        match slot {
            TemporalSlot::Previous => self.temporal_goals.previous = Some(goal_state),
            TemporalSlot::Current => self.temporal_goals.current = goal_state,
            TemporalSlot::Next => self.temporal_goals.next = Some(goal_state),
        }
    }
}
