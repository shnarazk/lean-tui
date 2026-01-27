//! Application state for the TUI.

use std::mem;

use async_lsp::lsp_types::Url;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    prelude::*,
    widgets::{Block, Paragraph},
    Frame,
};

use super::{
    modes::{
        BeforeAfterModeInput, DeductionTreeModeInput, DisplayMode, GoalTreeModeInput,
        StepsModeInput,
    },
    widgets::{KeyMouseEvent, SelectableItem},
};
use crate::{
    lean_rpc::{Goal, GotoLocation, GotoLocations, PaperproofStep},
    tui::widgets::{
        help_menu::HelpMenu,
        interactive_widget::InteractiveWidget,
        status_bar::{StatusBar, StatusBarInput},
    },
    tui_ipc::{
        socket_path, CaseSplitInfo, Command, CursorInfo, DefinitionInfo, GoalResult, Message,
        Position, ProofStep, TemporalSlot,
    },
};

/// Kind of navigation (definition or type definition).
#[derive(Debug, Clone, Copy, Default)]
pub enum NavigationKind {
    #[default]
    Definition,
    TypeDefinition,
}

/// A step in the local proof history (includes goals, used for navigation).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Will be used in Phase 2-5 for proof history tracking
pub struct LocalProofStep {
    /// Position in the source file where this step occurs.
    pub position: Position,
    /// The tactic text (if extractable).
    pub tactic: Option<String>,
    /// Goals at this proof step.
    pub goals: Vec<Goal>,
    /// Nesting depth (for have/cases scopes).
    pub scope_depth: usize,
}

/// Proof history tracking all steps in the current proof.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Will be used in Phase 2-5 for proof history tracking
pub struct ProofHistory {
    /// All proof steps, ordered by position.
    pub steps: Vec<LocalProofStep>,
    /// Index of the currently selected step.
    pub current_step_index: usize,
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
    /// Current display mode.
    display_mode: DisplayMode,
    /// Proof history for Paperproof view.
    #[allow(dead_code)] // Will be used in Phase 2-5 for proof history tracking
    proof_history: ProofHistory,
    /// Proof steps from Paperproof (if available).
    paperproof_steps: Option<Vec<PaperproofStep>>,
    /// Unified proof steps (from Paperproof or local tree-sitter analysis).
    proof_steps: Vec<ProofStep>,
    /// Index of current step (closest to cursor).
    current_step_index: usize,
    /// Status bar component.
    status_bar: StatusBar,
    /// Help menu overlay.
    help_menu: HelpMenu,
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
                paperproof_steps,
                proof_steps,
                current_step_index,
            } => {
                let line_changed = self.temporal_goals.current.position.line != position.line;

                // Keep previous definition if new one is None and we're on the same line
                let new_definition = if definition.is_none() && !line_changed {
                    self.definition.clone()
                } else {
                    definition
                };

                // Always update goals to ensure goto_locations are updated
                // (they may be resolved asynchronously after initial goal fetch)
                self.temporal_goals.current = GoalState {
                    goals,
                    position,
                    status: LoadStatus::Ready,
                };
                self.definition = new_definition;
                self.case_splits = case_splits;
                self.paperproof_steps = paperproof_steps;
                self.proof_steps = proof_steps;
                self.current_step_index = current_step_index;
                self.connected = true;
                self.error = None;
                self.refresh_temporal_columns();
            }
            Message::PaperproofData {
                uri: _,
                position: _,
                output,
            } => {
                self.paperproof_steps = if output.steps.is_empty() {
                    None
                } else {
                    Some(output.steps)
                };
                self.connected = true;
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

    /// Cycle to the next display mode.
    pub fn next_mode(&mut self) {
        self.display_mode.next();
    }

    /// Cycle to the previous display mode.
    pub fn prev_mode(&mut self) {
        self.display_mode.prev();
    }

    /// Navigate to the given selection (go to definition).
    pub fn navigate_to_selection(&mut self, selection: Option<SelectableItem>) {
        self.navigate_to_selection_with_kind(selection, NavigationKind::Definition);
    }

    /// Navigate to the given selection with a specific navigation kind.
    pub fn navigate_to_selection_with_kind(
        &mut self,
        selection: Option<SelectableItem>,
        kind: NavigationKind,
    ) {
        let Some(cursor) = &self.cursor else {
            return;
        };

        let goals_pos = self.goals_position().unwrap_or(cursor.position);
        let cmd = self.build_navigation_command(selection, cursor.uri.clone(), goals_pos, kind);
        self.queue_command(cmd);
    }

    fn build_navigation_command(
        &self,
        selection: Option<SelectableItem>,
        uri: Url,
        position: Position,
        kind: NavigationKind,
    ) -> Command {
        let goto_locations: Option<&GotoLocations> = match selection {
            Some(SelectableItem::Hypothesis { goal_idx, hyp_idx }) => self
                .goals()
                .get(goal_idx)
                .and_then(|g| g.hyps.get(hyp_idx))
                .map(|h| &h.goto_locations),
            Some(SelectableItem::GoalTarget { goal_idx }) => {
                self.goals().get(goal_idx).map(|g| &g.goto_locations)
            }
            None => None,
        };

        let goto_location: Option<&GotoLocation> = goto_locations.and_then(|locs| match kind {
            NavigationKind::Definition => locs.definition.as_ref(),
            NavigationKind::TypeDefinition => locs.type_def.as_ref(),
        });

        goto_location.map_or(Command::Navigate { uri, position }, |loc| {
            Command::Navigate {
                uri: loc.uri.clone(),
                position: loc.position,
            }
        })
    }

    fn refresh_temporal_columns(&mut self) {
        if self.display_mode.show_previous() {
            self.request_temporal_goals(TemporalSlot::Previous);
        }
        if self.display_mode.show_next() {
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

    /// Update all components with current state.
    pub fn update(&mut self) {
        // Fetch temporal goals if needed
        if self.display_mode.show_previous() && self.temporal_goals.previous.is_none() {
            self.request_temporal_goals(TemporalSlot::Previous);
        }
        if self.display_mode.show_next() && self.temporal_goals.next.is_none() {
            self.request_temporal_goals(TemporalSlot::Next);
        }

        self.update_display_mode();
        self.status_bar.update(StatusBarInput {
            filters: self.display_mode.filters(),
            keybindings: self.display_mode.keybindings(),
            supported_filters: self.display_mode.supported_filters(),
        });
    }

    fn update_display_mode(&mut self) {
        self.display_mode.update_goal_tree(GoalTreeModeInput {
            goals: self.goals().to_vec(),
            definition: self.definition.clone(),
            case_splits: self.case_splits.clone(),
            error: self.error.clone(),
        });
        self.display_mode.update_before_after(BeforeAfterModeInput {
            previous_goals: self
                .display_mode
                .show_previous()
                .then(|| {
                    self.temporal_goals
                        .previous
                        .as_ref()
                        .map(|g| g.goals.clone())
                })
                .flatten(),
            current_goals: self.goals().to_vec(),
            next_goals: self
                .display_mode
                .show_next()
                .then(|| self.temporal_goals.next.as_ref().map(|g| g.goals.clone()))
                .flatten(),
            definition: self.definition.clone(),
            error: self.error.clone(),
        });
        self.display_mode.update_steps(StepsModeInput {
            goals: self.goals().to_vec(),
            definition: self.definition.clone(),
            error: self.error.clone(),
            proof_steps: self.proof_steps.clone(),
            current_step_index: self.current_step_index,
            paperproof_steps: self.paperproof_steps.clone(),
        });
        self.display_mode
            .update_deduction_tree(DeductionTreeModeInput {
                goals: self.goals().to_vec(),
                definition: self.definition.clone(),
                error: self.error.clone(),
                current_step_index: self.current_step_index,
                paperproof_steps: self.paperproof_steps.clone(),
            });
    }

    /// Render the entire UI.
    pub fn render(&mut self, frame: &mut Frame) {
        let [main_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

        self.render_main(frame, main_area);
        self.status_bar.render(frame, status_area);
        self.help_menu.render(frame, frame.area());
    }

    fn render_main(&mut self, frame: &mut Frame, area: Rect) {
        let title = self.build_title();
        let backends = format!(" {} ", self.display_mode.backends_display());
        let position_info = self.build_position_info();

        let block = Block::bordered()
            .title(title)
            .title_top(Line::from(backends).right_aligned())
            .title_bottom(Line::from(position_info).right_aligned())
            .border_style(Style::new().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if !self.connected {
            frame.render_widget(
                Paragraph::new(format!("Connecting to {}...", socket_path().display())),
                inner,
            );
            return;
        }

        self.display_mode.render(frame, inner);
    }

    fn build_title(&self) -> String {
        if let (Some(def), Some(cursor)) = (&self.definition, &self.cursor) {
            let filename = cursor.filename().unwrap_or("?");
            format!(" {} {} ({}) ", def.kind, def.name, filename)
        } else if let Some(cursor) = &self.cursor {
            let filename = cursor.filename().unwrap_or("?");
            format!(" lean-tui [{}] ({}) ", self.display_mode.name(), filename)
        } else {
            format!(" lean-tui [{}] ", self.display_mode.name())
        }
    }

    fn build_position_info(&self) -> String {
        self.cursor.as_ref().map_or(String::new(), |cursor| {
            format!(
                " {}:{} ({}) ",
                cursor.position.line + 1,
                cursor.position.character + 1,
                cursor.method
            )
        })
    }

    /// Handle crossterm events.
    pub fn handle_event(&mut self, event: &Event) {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if self.help_menu.handle_event(*key) {
                    return;
                }
                if !self.handle_global_key(key.code) {
                    self.display_mode.handle_event(KeyMouseEvent::Key(*key));
                }
            }
            Event::Mouse(mouse) => {
                self.display_mode.handle_event(KeyMouseEvent::Mouse(*mouse));
            }
            _ => {}
        }
    }

    fn handle_global_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') => {
                self.should_exit = true;
                true
            }
            KeyCode::Char('?') => {
                self.help_menu.toggle();
                true
            }
            KeyCode::Char(']') => {
                self.next_mode();
                true
            }
            KeyCode::Char('[') => {
                self.prev_mode();
                true
            }
            KeyCode::Char('d') | KeyCode::Enter => {
                let selection = self.display_mode.current_selection();
                self.navigate_to_selection(selection);
                true
            }
            KeyCode::Char('t') => {
                let selection = self.display_mode.current_selection();
                self.navigate_to_selection_with_kind(selection, NavigationKind::TypeDefinition);
                true
            }
            _ => false,
        }
    }
}
