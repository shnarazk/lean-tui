//! TUI module for displaying Lean proof goals.

mod app;
mod ui;

use std::{io::stdout, time::Duration};

use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use ui::render;

use crate::{error::Result, tui_ipc::spawn_socket_handler};

/// Run the TUI application.
pub fn run() -> Result<()> {
    // Initialize terminal with mouse support
    enable_raw_mode()?;
    stdout()
        .execute(EnterAlternateScreen)?
        .execute(EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Spawn socket handler and create app
    let mut socket = spawn_socket_handler();
    let mut app = App::new();

    // Main event loop
    while !app.should_exit {
        // Process all pending messages from proxy
        while let Ok(msg) = socket.rx.try_recv() {
            app.handle_message(msg);
        }

        // Send any pending navigation commands
        if let Some(cmd) = app.take_pending_navigation() {
            // Use try_send to avoid blocking
            let _ = socket.tx.try_send(cmd);
        }

        // Render UI (updates click regions)
        terminal.draw(|frame| render(frame, &mut app))?;

        // Poll for terminal events
        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            app.handle_event(&event);
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    stdout()
        .execute(DisableMouseCapture)?
        .execute(LeaveAlternateScreen)?;

    Ok(())
}
