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
        BeforeAfterModeInput, DeductionTreeModeInput, DisplayMode, PlainListInput, StepsModeInput,
    },
    widgets::{welcome::WelcomeScreen, KeyMouseEvent, Selection},
};
use crate::{
    lean_rpc::{Goal, GotoLocation},
    tui::widgets::{
        help_menu::{HelpMenu, HelpMenuWidget},
        status_bar::{StatusBar, StatusBarInput, StatusBarWidget},
        InteractiveStatefulWidget,
    },
    tui_ipc::{
        socket_path, Command, CursorInfo, DefinitionInfo, GoalResult, Message, Position, ProofDag,
        ProofDagSource, TemporalSlot,
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
    /// Unified proof DAG - single source of truth for all display modes.
    proof_dag: Option<ProofDag>,
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
                proof_dag,
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
                self.proof_dag = proof_dag;
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

    /// Cycle to the next display mode.
    pub fn next_mode(&mut self) {
        self.display_mode.next();
    }

    /// Cycle to the previous display mode.
    pub fn prev_mode(&mut self) {
        self.display_mode.prev();
    }

    /// Navigate to the given selection (go to definition).
    pub fn navigate_to_selection(&mut self, selection: Option<Selection>) {
        self.navigate_to_selection_with_kind(selection, NavigationKind::Definition);
    }

    /// Navigate to the given selection with a specific navigation kind.
    pub fn navigate_to_selection_with_kind(
        &mut self,
        selection: Option<Selection>,
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
        selection: Option<Selection>,
        uri: Url,
        position: Position,
        kind: NavigationKind,
    ) -> Command {
        let goto_location = self.resolve_goto_location(selection, kind);

        tracing::debug!(
            "Navigation: selection={:?}, kind={:?}, goto_location={:?}",
            selection,
            kind,
            goto_location.map(|l| (&l.uri, l.position))
        );

        goto_location.map_or(Command::Navigate { uri, position }, |loc| {
            Command::Navigate {
                uri: loc.uri.clone(),
                position: loc.position,
            }
        })
    }

    fn resolve_goto_location(
        &self,
        selection: Option<Selection>,
        kind: NavigationKind,
    ) -> Option<&GotoLocation> {
        let dag = self.proof_dag.as_ref()?;

        let locations = match selection? {
            Selection::InitialHyp { hyp_idx } => dag
                .initial_state
                .hypotheses
                .get(hyp_idx)
                .map(|h| &h.goto_locations),
            Selection::Hyp { node_id, hyp_idx } => dag
                .get(node_id)
                .and_then(|node| node.state_after.hypotheses.get(hyp_idx))
                .map(|h| &h.goto_locations),
            Selection::Goal { node_id, goal_idx } => dag
                .get(node_id)
                .and_then(|node| node.state_after.goals.get(goal_idx))
                .map(|g| &g.goto_locations),
            Selection::Theorem => dag.initial_state.goals.first().map(|g| &g.goto_locations),
        }?;

        match kind {
            NavigationKind::Definition => locations.definition.as_ref(),
            NavigationKind::TypeDefinition => locations.type_def.as_ref(),
        }
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
        StatusBarWidget::update_state(
            &mut self.status_bar,
            StatusBarInput {
                filters: self.display_mode.filters(),
                keybindings: self.display_mode.keybindings(),
                supported_filters: self.display_mode.supported_filters(),
            },
        );
    }

    fn update_display_mode(&mut self) {
        self.display_mode.update_open_goal_list(PlainListInput {
            goals: self.goals().to_vec(),
            definition: self.definition.clone(),
            error: self.error.clone(),
            proof_dag: self.proof_dag.clone(),
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
            proof_dag: self.proof_dag.clone(),
        });
        self.display_mode.update_steps(StepsModeInput {
            goals: self.goals().to_vec(),
            definition: self.definition.clone(),
            error: self.error.clone(),
            proof_dag: self.proof_dag.clone(),
        });
        self.display_mode
            .update_deduction_tree(DeductionTreeModeInput {
                goals: self.goals().to_vec(),
                definition: self.definition.clone(),
                error: self.error.clone(),
                proof_dag: self.proof_dag.clone(),
            });
    }

    /// Render the entire UI.
    pub fn render(&mut self, frame: &mut Frame) {
        let [main_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

        self.render_main(frame, main_area);
        frame.render_stateful_widget(StatusBarWidget, status_area, &mut self.status_bar);
        frame.render_stateful_widget(HelpMenuWidget, frame.area(), &mut self.help_menu);
    }

    fn render_main(&mut self, frame: &mut Frame, area: Rect) {
        // Show welcome screen when connected but no cursor position yet
        let show_welcome = self.connected && self.cursor.is_none();

        if show_welcome {
            frame.render_widget(WelcomeScreen, area);
            return;
        }

        let title = self.build_title();
        let mode_name = format!(" {} ", self.display_mode.name());
        let backend = self.build_backend_display();
        let position_info = self.build_position_info();

        let block = Block::bordered()
            .title(title)
            .title_top(Line::from(mode_name).right_aligned())
            .title_bottom(Line::from(backend).left_aligned())
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

    fn build_backend_display(&self) -> String {
        let source_name = self.proof_dag.as_ref().map(|dag| match dag.source {
            ProofDagSource::Paperproof => "Paperproof",
            ProofDagSource::Local => "tree-sitter",
        });
        source_name.map_or(String::new(), |name| format!(" {name} "))
    }

    /// Handle crossterm events.
    pub fn handle_event(&mut self, event: &Event) {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if HelpMenuWidget::handle_event(&mut self.help_menu, *key) {
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
