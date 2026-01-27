//! TUI for displaying Lean proof goals.

pub mod app;
mod components;

use std::{io::stdout, time::Duration};

use app::{App, ViewMode};
use components::{
    Component, GoalView, GoalViewInput, Header, HelpMenu, KeyMouseEvent, KeyPress,
    PaperproofView, PaperproofViewInput, StatusBar,
};
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
use tokio::time::sleep;

use crate::{
    error::Result,
    tui_ipc::{socket_path, spawn_socket_handler},
};

pub async fn run() -> Result<()> {
    enable_raw_mode()?;
    stdout()
        .execute(EnterAlternateScreen)?
        .execute(EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut socket = spawn_socket_handler();
    let mut app = App::default();

    // Create components
    let mut header = Header::default();
    let mut goal_view = GoalView::default();
    let mut paperproof_view = PaperproofView::default();
    let mut status_bar = StatusBar::default();
    let mut help_menu = HelpMenu::default();

    let mut event_stream = EventStream::new();

    while !app.should_exit {
        header.update(app.cursor.clone());
        goal_view.update(GoalViewInput {
            goals: app.goals().to_vec(),
            definition: app.definition.clone(),
            case_splits: app.case_splits.clone(),
            error: app.error.clone(),
        });
        paperproof_view.update(PaperproofViewInput {
            goals: app.goals().to_vec(),
            definition: app.definition.clone(),
            error: app.error.clone(),
            proof_steps: app.proof_steps.clone(),
            current_step_index: app.current_step_index,
        });
        status_bar.update(goal_view.filters());

        terminal.draw(|frame| {
            render_frame(
                frame,
                &app,
                &mut header,
                &mut goal_view,
                &mut paperproof_view,
                &mut status_bar,
                &mut help_menu,
            );
        })?;

        tokio::select! {
            Some(msg) = socket.rx.recv() => {
                app.handle_message(msg);
            }
            Some(Ok(event)) = event_stream.next() => {
                match &event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if help_menu.handle_event(KeyPress(*key)) {
                            continue;
                        }
                        if !handle_global_event(&mut app, &mut help_menu, &goal_view, &paperproof_view, &event) {
                            match app.view_mode {
                                ViewMode::Standard => {
                                    goal_view.handle_event(KeyMouseEvent::Key(*key));
                                }
                                ViewMode::Paperproof => {
                                    paperproof_view.handle_event(KeyMouseEvent::Key(*key));
                                }
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        match app.view_mode {
                            ViewMode::Standard => {
                                goal_view.handle_event(KeyMouseEvent::Mouse(*mouse));
                            }
                            ViewMode::Paperproof => {
                                paperproof_view.handle_event(KeyMouseEvent::Mouse(*mouse));
                            }
                        }
                    }
                    _ => {}
                }
            }
            () = sleep(Duration::from_millis(50)) => {
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
    paperproof_view: &PaperproofView,
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
        KeyCode::Char('v') => {
            app.toggle_view_mode();
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
            match app.view_mode {
                ViewMode::Standard => app.navigate_to_selection(goal_view.current_selection()),
                ViewMode::Paperproof => app.navigate_to_selection(paperproof_view.current_selection()),
            }
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
    paperproof_view: &mut PaperproofView,
    status_bar: &mut StatusBar,
    help_menu: &mut HelpMenu,
) {
    let [main_area, help_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    render_main(frame, app, main_area, header, goal_view, paperproof_view);
    status_bar.render(frame, help_area);

    help_menu.render(frame, frame.area());
}

fn render_main(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    header: &mut Header,
    goal_view: &mut GoalView,
    paperproof_view: &mut PaperproofView,
) {
    let title = match app.view_mode {
        ViewMode::Standard => " lean-tui ",
        ViewMode::Paperproof => " lean-tui [paperproof] ",
    };
    let block = Block::bordered()
        .title(title)
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

    match app.view_mode {
        ViewMode::Standard => goal_view.render(frame, content_area),
        ViewMode::Paperproof => paperproof_view.render(frame, content_area),
    }
}
