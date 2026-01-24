//! Application state and event handling.

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::widgets::ListState;

use crate::{
    lake_ipc::Goal,
    tui_ipc::{CursorInfo, Message, Position},
};

/// A selectable item in the TUI (hypothesis or goal target).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectableItem {
    Hypothesis { goal_idx: usize, hyp_idx: usize },
    GoalTarget { goal_idx: usize },
}

/// Application state.
#[derive(Default)]
pub struct App {
    /// Current cursor position from editor.
    pub cursor: CursorInfo,
    /// Goals at current position.
    pub goals: Vec<Goal>,
    /// Position where goals were fetched.
    pub goals_position: Option<Position>,
    /// Current error message.
    pub error: Option<String>,
    /// Whether connected to proxy.
    pub connected: bool,
    /// Selection state for the goal list.
    pub list_state: ListState,
    /// Whether app should exit.
    pub should_exit: bool,
}

impl App {
    /// Create a new app instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all selectable items as a flat list.
    pub fn selectable_items(&self) -> Vec<SelectableItem> {
        let mut items = Vec::new();
        for (goal_idx, goal) in self.goals.iter().enumerate() {
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
                self.cursor = cursor;
                self.connected = true;
                self.error = None;
            }
            Message::Goals {
                uri: _,
                position,
                goals,
            } => {
                self.goals = goals;
                self.goals_position = Some(position);
                self.connected = true;
                self.error = None;
                // Reset selection when goals change
                if self.selectable_items().is_empty() {
                    self.list_state.select(None);
                } else {
                    self.list_state.select(Some(0));
                }
            }
            Message::Error { error } => {
                self.error = Some(error);
                self.connected = true;
            }
        }
    }

    /// Handle terminal event. Returns true if event was handled.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::Key(key) = event {
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
                    // TODO: Trigger navigation to selected item
                    if let Some(selection) = self.current_selection() {
                        tracing::info!("Selected: {:?}", selection);
                    }
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }
}
