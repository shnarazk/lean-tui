//! Application state for the TUI.

use std::{io::stdout, mem};

use async_lsp::lsp_types::Url;
use crossterm::{
    clipboard::CopyToClipboard,
    event::{Event, KeyCode, KeyEventKind},
    ExecutableCommand,
};
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
    lean_rpc::{GotoLocation, ProofDag, ProofState},
    tui::widgets::{
        help_menu::{HelpMenu, HelpMenuWidget},
        status_bar::{StatusBar, StatusBarInput, StatusBarWidget},
        InteractiveStatefulWidget,
    },
    tui_ipc::{socket_path, Command, CursorInfo, Message, Position},
};

/// Kind of navigation (definition or type definition).
#[derive(Debug, Clone, Copy, Default)]
pub enum NavigationKind {
    #[default]
    Definition,
    TypeDefinition,
}

/// Information about the enclosing definition (theorem, lemma, def, etc.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionInfo {
    /// Kind of definition (theorem, lemma, def, example)
    pub kind: Option<String>,
    /// Name of the definition
    pub name: String,
    /// Line where the definition starts
    pub line: Option<u32>,
}

/// Application state.
#[derive(Default)]
pub struct App {
    /// Current cursor position from editor.
    pub cursor: Option<CursorInfo>,
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
    /// Unified proof DAG - single source of truth for all display modes.
    proof_dag: Option<ProofDag>,
    /// Status bar component.
    status_bar: StatusBar,
    /// Help menu overlay.
    help_menu: HelpMenu,
}

impl App {
    /// Get current proof state from the DAG.
    pub fn proof_state(&self) -> ProofState {
        self.proof_dag
            .as_ref()
            .and_then(|dag| dag.current_node)
            .and_then(|id| self.proof_dag.as_ref()?.get(id))
            .map(|node| node.state_after.clone())
            .unwrap_or_default()
    }

    /// Get position where current goals were fetched.
    pub fn goals_position(&self) -> Option<Position> {
        self.proof_dag
            .as_ref()
            .and_then(|dag| dag.current_node)
            .and_then(|id| self.proof_dag.as_ref()?.get(id))
            .map(|node| node.position)
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
            Message::ProofDag {
                uri: _,
                position: _,
                proof_dag,
            } => {
                // Extract definition name from the ProofDag
                let definition_name = proof_dag
                    .as_ref()
                    .and_then(|dag| dag.definition_name.clone());

                self.definition = definition_name.map(|name| DefinitionInfo {
                    name,
                    kind: None,
                    line: None,
                });
                self.proof_dag = proof_dag;
                self.connected = true;
                self.error = None;
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

    /// Get the text of the currently selected item (hypothesis or goal).
    fn get_selection_text(&self, selection: Option<Selection>) -> Option<String> {
        let dag = self.proof_dag.as_ref()?;

        match selection? {
            Selection::InitialHyp { hyp_idx } => dag
                .initial_state
                .hypotheses
                .get(hyp_idx)
                .map(|h| format!("{} : {}", h.name, h.type_.to_plain_text())),
            Selection::Hyp { node_id, hyp_idx } => dag
                .get(node_id)
                .and_then(|node| node.state_after.hypotheses.get(hyp_idx))
                .map(|h| format!("{} : {}", h.name, h.type_.to_plain_text())),
            Selection::Goal { node_id, goal_idx } => dag
                .get(node_id)
                .and_then(|node| node.state_after.goals.get(goal_idx))
                .map(|g| g.type_.to_plain_text()),
            Selection::Theorem => dag
                .initial_state
                .goals
                .first()
                .map(|g| g.type_.to_plain_text()),
        }
    }

    /// Copy the selected item's text to the clipboard.
    fn copy_selection_to_clipboard(&self) {
        let selection = self.display_mode.current_selection();
        if let Some(text) = self.get_selection_text(selection) {
            let _ = stdout().execute(CopyToClipboard::to_clipboard_from(text));
        }
    }

    /// Update all components with current state.
    pub fn update(&mut self) {
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
            state: self.proof_state(),
            definition: self.definition.clone(),
            error: self.error.clone(),
            proof_dag: self.proof_dag.clone(),
        });
        // Derive before/after states from the ProofDag
        let current_node = self
            .proof_dag
            .as_ref()
            .and_then(|dag| dag.current_node)
            .and_then(|id| self.proof_dag.as_ref()?.get(id));

        let previous_state = self
            .display_mode
            .show_previous()
            .then(|| current_node.map(|n| n.state_before.clone()))
            .flatten();

        let next_state = self
            .display_mode
            .show_next()
            .then(|| {
                current_node
                    .and_then(|n| n.children.first())
                    .and_then(|&child_id| self.proof_dag.as_ref()?.get(child_id))
                    .map(|child| child.state_after.clone())
            })
            .flatten();

        self.display_mode.update_before_after(BeforeAfterModeInput {
            previous_state,
            current_state: current_node
                .map(|n| n.state_after.clone())
                .unwrap_or_default(),
            next_state,
            definition: self.definition.clone(),
            error: self.error.clone(),
            proof_dag: self.proof_dag.clone(),
        });
        self.display_mode.update_steps(StepsModeInput {
            state: self.proof_state(),
            definition: self.definition.clone(),
            error: self.error.clone(),
            proof_dag: self.proof_dag.clone(),
        });
        self.display_mode
            .update_deduction_tree(DeductionTreeModeInput {
                state: self.proof_state(),
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
            let kind = def.kind.as_deref().unwrap_or("proof");
            format!(" {kind} {} ({}) ", def.name, filename)
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
        if self.proof_dag.is_some() {
            " Server ".to_string()
        } else {
            String::new()
        }
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
            KeyCode::Char('y') => {
                self.copy_selection_to_clipboard();
                true
            }
            _ => false,
        }
    }
}
