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
use socket::spawn_socket_reader;
use ui::render;

use crate::error::Result;

/// Run the TUI application.
pub fn run() -> Result<()> {
    // Initialize terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Spawn socket reader and create app
    let mut rx = spawn_socket_reader();
    let mut app = App::new();

    // Main event loop
    while !app.should_exit {
        // Process all pending messages from proxy
        while let Ok(msg) = rx.try_recv() {
            app.handle_message(msg);
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
