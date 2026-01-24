use std::io::stdout;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use crate::cursor::{CursorInfo, Message, SOCKET_PATH};
use crate::error::Result;

/// TUI application state
#[derive(Default)]
struct AppState {
    cursor: CursorInfo,
    connected: bool,
}

impl AppState {
    fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Cursor(cursor) => {
                self.cursor = cursor;
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

    // Channel for messages from proxy
    let (tx, mut rx) = mpsc::channel::<Message>(16);

    // Spawn socket reader task
    tokio::spawn(async move {
        loop {
            match UnixStream::connect(SOCKET_PATH).await {
                Ok(stream) => {
                    let reader = BufReader::new(stream);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                            if tx.send(msg).await.is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(_) => {
                    // Retry connection after delay
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

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

fn draw_ui(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    let block = Block::default()
        .title(" lean-tui ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let content = if state.connected {
        format!(
            "File: {}\nLine: {}\nColumn: {}\nMethod: {}",
            state.cursor.filename(),
            state.cursor.line() + 1, // 1-indexed for display
            state.cursor.character() + 1,
            state.cursor.method
        )
    } else {
        format!("Connecting to {}...", SOCKET_PATH)
    };

    let paragraph = Paragraph::new(content)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, area);
}
