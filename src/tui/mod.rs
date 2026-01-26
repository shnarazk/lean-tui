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
use ratatui::{layout::Rect, prelude::*};
use ui::{compute_click_regions, render};

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
        // Compute click regions before rendering (uses same layout logic)
        let size = terminal.size()?;
        let rect = Rect::new(0, 0, size.width, size.height);
        app.click_regions = compute_click_regions(&app, rect);

        // Render UI (pure function, no mutation)
        terminal.draw(|frame| render(frame, &app))?;

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
                for cmd in app.take_commands() {
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
