mod draw;
mod socket;

use std::io::stdout;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;

use crate::error::Result;
use crate::lake_ipc::Goal;
use crate::tui_ipc::{CursorInfo, Message, Position};
use draw::draw_ui;
use socket::spawn_socket_reader;

/// TUI application state
#[derive(Default)]
pub(crate) struct AppState {
    pub cursor: CursorInfo,
    pub goals: Vec<Goal>,
    pub goals_position: Option<Position>,
    pub error: Option<String>,
    pub connected: bool,
}

impl AppState {
    fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Cursor(cursor) => {
                self.cursor = cursor;
                self.connected = true;
                self.error = None; // Clear errors on new cursor position
            }
            Message::Goals {
                uri: _,
                position,
                goals,
            } => {
                self.goals = goals;
                self.goals_position = Some(position);
                self.connected = true;
                self.error = None; // Clear errors on successful goals
            }
            Message::Error { error } => {
                self.error = Some(error);
                self.connected = true;
            }
        }
    }
}

pub async fn run() -> Result<()> {
    // Set up terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Spawn socket reader task and get receiver
    let mut rx = spawn_socket_reader();

    let mut state = AppState::default();

    loop {
        // Process all pending messages
        while let Ok(msg) = rx.try_recv() {
            state.handle_message(msg);
        }

        // Draw UI
        terminal.draw(|frame| draw_ui(frame, &state))?;

        // Handle input (with timeout for responsiveness)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
