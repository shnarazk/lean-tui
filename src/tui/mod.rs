//! TUI module for displaying Lean proof goals.

mod app;
mod ui;

use std::{io::stdout, time::Duration};

use app::App;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, EventStream},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures::StreamExt;
use ratatui::prelude::*;
use ui::render;

use crate::{error::Result, tui_ipc::spawn_socket_handler};

/// Run the TUI application.
pub async fn run() -> Result<()> {
    // Initialize terminal with mouse support
    enable_raw_mode()?;
    stdout()
        .execute(EnterAlternateScreen)?
        .execute(EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Spawn socket handler and create app
    let mut socket = spawn_socket_handler();
    let mut app = App::default();

    // Event stream for async terminal events
    let mut event_stream = EventStream::new();

    // Main event loop
    while !app.should_exit {
        // Render UI (updates click regions)
        terminal.draw(|frame| render(frame, &mut app))?;

        // Wait for events with timeout
        tokio::select! {
            // Receive messages from proxy
            Some(msg) = socket.rx.recv() => {
                app.handle_message(msg);
            }
            // Handle terminal events
            Some(Ok(event)) = event_stream.next() => {
                app.handle_event(&event);
            }
            // Send pending commands (with small delay to batch)
            () = tokio::time::sleep(Duration::from_millis(50)) => {
                if let Some(cmd) = app.take_pending_navigation() {
                    let _ = socket.tx.send(cmd).await;
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    stdout()
        .execute(DisableMouseCapture)?
        .execute(LeaveAlternateScreen)?;

    Ok(())
}
