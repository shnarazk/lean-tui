//! TUI for displaying Lean proof goals.

pub mod app;
mod components;
mod modes;

use std::{io::stdout, time::Duration};

use app::{App, NavigationKind};
use components::{Component, Header, HelpMenu, KeyMouseEvent, KeyPress, StatusBar, StatusBarInput};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures::StreamExt;
use modes::{
    BeforeAfterMode, BeforeAfterModeInput, DeductionTreeMode, DeductionTreeModeInput, DisplayMode,
    GoalTreeMode, GoalTreeModeInput, Mode, StepsMode, StepsModeInput,
};
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

#[allow(clippy::too_many_lines)]
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
    let mut goal_tree_mode = GoalTreeMode::default();
    let mut before_after_mode = BeforeAfterMode::default();
    let mut steps_mode = StepsMode::default();
    let mut deduction_tree_mode = DeductionTreeMode::default();
    let mut status_bar = StatusBar::default();
    let mut help_menu = HelpMenu::default();

    let mut event_stream = EventStream::new();

    while !app.should_exit {
        header.update(app.cursor.clone());

        // Update all modes with current state
        goal_tree_mode.update(GoalTreeModeInput {
            goals: app.goals().to_vec(),
            definition: app.definition.clone(),
            case_splits: app.case_splits.clone(),
            error: app.error.clone(),
        });
        before_after_mode.update(BeforeAfterModeInput {
            previous_goals: app
                .columns
                .previous
                .then(|| {
                    app.temporal_goals
                        .previous
                        .as_ref()
                        .map(|g| g.goals.clone())
                })
                .flatten(),
            current_goals: app.goals().to_vec(),
            next_goals: app
                .columns
                .next
                .then(|| app.temporal_goals.next.as_ref().map(|g| g.goals.clone()))
                .flatten(),
            definition: app.definition.clone(),
            error: app.error.clone(),
        });
        steps_mode.update(StepsModeInput {
            goals: app.goals().to_vec(),
            definition: app.definition.clone(),
            error: app.error.clone(),
            proof_steps: app.proof_steps.clone(),
            current_step_index: app.current_step_index,
            paperproof_steps: app.paperproof_steps.clone(),
        });
        deduction_tree_mode.update(DeductionTreeModeInput {
            goals: app.goals().to_vec(),
            definition: app.definition.clone(),
            error: app.error.clone(),
            current_step_index: app.current_step_index,
            paperproof_steps: app.paperproof_steps.clone(),
        });

        // Use filters from the active mode for status bar
        let filters = match app.display_mode {
            DisplayMode::GoalTree => goal_tree_mode.filters(),
            DisplayMode::BeforeAfter => before_after_mode.filters(),
            DisplayMode::StepsView => steps_mode.filters(),
            DisplayMode::DeductionTree => deduction_tree_mode.filters(),
        };
        status_bar.update(StatusBarInput {
            filters,
            display_mode: app.display_mode,
            supported_filters: app.display_mode.supported_filters(),
        });

        terminal.draw(|frame| {
            render_frame(
                frame,
                &app,
                &mut header,
                &mut goal_tree_mode,
                &mut before_after_mode,
                &mut steps_mode,
                &mut deduction_tree_mode,
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
                        if !handle_global_event(
                            &mut app,
                            &mut help_menu,
                            &goal_tree_mode,
                            &before_after_mode,
                            &steps_mode,
                            &deduction_tree_mode,
                            &event,
                        ) {
                            match app.display_mode {
                                DisplayMode::GoalTree => {
                                    goal_tree_mode.handle_event(KeyMouseEvent::Key(*key));
                                }
                                DisplayMode::BeforeAfter => {
                                    before_after_mode.handle_event(KeyMouseEvent::Key(*key));
                                }
                                DisplayMode::StepsView => {
                                    steps_mode.handle_event(KeyMouseEvent::Key(*key));
                                }
                                DisplayMode::DeductionTree => {
                                    deduction_tree_mode.handle_event(KeyMouseEvent::Key(*key));
                                }
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        match app.display_mode {
                            DisplayMode::GoalTree => {
                                goal_tree_mode.handle_event(KeyMouseEvent::Mouse(*mouse));
                            }
                            DisplayMode::BeforeAfter => {
                                before_after_mode.handle_event(KeyMouseEvent::Mouse(*mouse));
                            }
                            DisplayMode::StepsView => {
                                steps_mode.handle_event(KeyMouseEvent::Mouse(*mouse));
                            }
                            DisplayMode::DeductionTree => {
                                deduction_tree_mode.handle_event(KeyMouseEvent::Mouse(*mouse));
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
    goal_tree_mode: &GoalTreeMode,
    before_after_mode: &BeforeAfterMode,
    steps_mode: &StepsMode,
    deduction_tree_mode: &DeductionTreeMode,
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
        KeyCode::Char(']') => {
            app.next_mode();
            true
        }
        KeyCode::Char('[') => {
            app.prev_mode();
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
        KeyCode::Char('d') | KeyCode::Enter => {
            let selection = match app.display_mode {
                DisplayMode::GoalTree => goal_tree_mode.current_selection(),
                DisplayMode::BeforeAfter => before_after_mode.current_selection(),
                DisplayMode::StepsView => steps_mode.current_selection(),
                DisplayMode::DeductionTree => deduction_tree_mode.current_selection(),
            };
            app.navigate_to_selection(selection);
            true
        }
        KeyCode::Char('t') => {
            let selection = match app.display_mode {
                DisplayMode::GoalTree => goal_tree_mode.current_selection(),
                DisplayMode::BeforeAfter => before_after_mode.current_selection(),
                DisplayMode::StepsView => steps_mode.current_selection(),
                DisplayMode::DeductionTree => deduction_tree_mode.current_selection(),
            };
            app.navigate_to_selection_with_kind(selection, NavigationKind::TypeDefinition);
            true
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_frame(
    frame: &mut Frame,
    app: &App,
    header: &mut Header,
    goal_tree_mode: &mut GoalTreeMode,
    before_after_mode: &mut BeforeAfterMode,
    steps_mode: &mut StepsMode,
    deduction_tree_mode: &mut DeductionTreeMode,
    status_bar: &mut StatusBar,
    help_menu: &mut HelpMenu,
) {
    let [main_area, help_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    render_main(
        frame,
        app,
        main_area,
        header,
        goal_tree_mode,
        before_after_mode,
        steps_mode,
        deduction_tree_mode,
    );
    status_bar.render(frame, help_area);

    help_menu.render(frame, frame.area());
}

#[allow(clippy::too_many_arguments)]
fn render_main(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    header: &mut Header,
    goal_tree_mode: &mut GoalTreeMode,
    before_after_mode: &mut BeforeAfterMode,
    steps_mode: &mut StepsMode,
    deduction_tree_mode: &mut DeductionTreeMode,
) {
    let title = format!(" lean-tui [{}] ", app.display_mode.name());
    let backends = format!(" {} ", app.display_mode.backends_display());
    let block = Block::bordered()
        .title(title)
        .title_top(Line::from(backends).right_aligned())
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

    match app.display_mode {
        DisplayMode::GoalTree => goal_tree_mode.render(frame, content_area),
        DisplayMode::BeforeAfter => before_after_mode.render(frame, content_area),
        DisplayMode::StepsView => steps_mode.render(frame, content_area),
        DisplayMode::DeductionTree => deduction_tree_mode.render(frame, content_area),
    }
}
