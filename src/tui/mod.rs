//! TUI module for displaying Lean proof goals.
//!
//! Architecture:
//! - `app.rs`: Application state and event handling
//! - `ui.rs`: UI rendering
//! - `socket.rs`: Unix socket connection to proxy

mod app;
mod socket;
mod ui;

use std::{io::stdout, time::Duration};

use app::App;
use crossterm::{
    event,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use socket::spawn_socket_handler;
use ui::render;

use crate::error::Result;

/// Run the TUI application.
pub fn run() -> Result<()> {
    // Initialize terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
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

        // Render UI
        terminal.draw(|frame| render(frame, &app))?;

        // Poll for terminal events
        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            app.handle_event(&event);
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
