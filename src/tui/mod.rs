//! TUI module for displaying Lean proof goals.

pub mod app;
mod components;

use std::{io::stdout, time::Duration};

use app::App;
use components::{Component, GoalView, Header, HelpMenu, StatusBar};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures::StreamExt;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    prelude::*,
    widgets::{Block, Paragraph},
};

use crate::{
    error::Result,
    tui_ipc::{socket_path, spawn_socket_handler},
};

/// Run the TUI application.
pub async fn run() -> Result<()> {
    enable_raw_mode()?;
    stdout()
        .execute(EnterAlternateScreen)?
        .execute(EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut socket = spawn_socket_handler();
    let mut app = App::default();

    // Create components
    let mut header = Header::new();
    let mut goal_view = GoalView::new();
    let mut status_bar = StatusBar::new();
    let mut help_menu = HelpMenu::new();

    let mut event_stream = EventStream::new();

    while !app.should_exit {
        // Sync component state from app
        header.set_cursor(app.cursor.clone());
        goal_view.set_goals(app.goals().to_vec());
        goal_view.set_definition(app.definition.clone());
        goal_view.set_case_splits(app.case_splits.clone());
        goal_view.set_error(app.error.clone());
        goal_view.set_filters(app.filters);
        status_bar.set_filters(app.filters);

        // Render UI
        terminal.draw(|frame| {
            render_frame(
                frame,
                &app,
                &mut header,
                &mut goal_view,
                &mut status_bar,
                &mut help_menu,
            );
        })?;

        // Update click regions from goal_view
        app.click_regions = goal_view.click_regions().to_vec();

        tokio::select! {
            Some(msg) = socket.rx.recv() => {
                app.handle_message(msg);
            }
            Some(Ok(event)) = event_stream.next() => {
                // Help menu handles its own events when visible
                if help_menu.handle_event(&event) {
                    continue;
                }
                // Handle global events
                if !handle_global_event(&mut app, &mut help_menu, &goal_view, &event) {
                    // Delegate to components
                    goal_view.handle_event(&event);
                    app.filters = goal_view.filters();
                }
            }
            () = tokio::time::sleep(Duration::from_millis(50)) => {
                for cmd in app.take_commands() {
                    let _ = socket.tx.send(cmd).await;
                }
            }
        }
    }

    disable_raw_mode()?;
    stdout()
        .execute(DisableMouseCapture)?
        .execute(LeaveAlternateScreen)?;

    Ok(())
}

fn handle_global_event(
    app: &mut App,
    help_menu: &mut HelpMenu,
    goal_view: &GoalView,
    event: &Event,
) -> bool {
    let Event::Key(key) = event else {
        return false;
    };
    if key.kind != KeyEventKind::Press {
        return false;
    }

    match key.code {
        KeyCode::Char('q') => {
            app.should_exit = true;
            true
        }
        KeyCode::Char('?') => {
            help_menu.toggle();
            true
        }
        KeyCode::Char('p') => {
            app.toggle_previous_column();
            true
        }
        KeyCode::Char('n') => {
            app.toggle_next_column();
            true
        }
        KeyCode::Enter => {
            app.navigate_to_selection(goal_view.current_selection());
            true
        }
        _ => false,
    }
}

fn render_frame(
    frame: &mut Frame,
    app: &App,
    header: &mut Header,
    goal_view: &mut GoalView,
    status_bar: &mut StatusBar,
    help_menu: &mut HelpMenu,
) {
    let [main_area, help_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    render_main(frame, app, main_area, header, goal_view);
    status_bar.render(frame, help_area);

    // Render help popup on top if visible
    help_menu.render(frame, frame.area());
}

fn render_main(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    header: &mut Header,
    goal_view: &mut GoalView,
) {
    let block = Block::bordered()
        .title(" lean-tui ")
        .border_style(Style::new().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if !app.connected {
        frame.render_widget(
            Paragraph::new(format!("Connecting to {}...", socket_path().display())),
            inner,
        );
        return;
    }

    let [header_area, content_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(inner);

    header.render(frame, header_area);
    goal_view.render(frame, content_area);
}
