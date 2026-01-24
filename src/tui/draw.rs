use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::{AppState, SelectableItem};
use crate::tui_ipc::SOCKET_PATH;

pub fn draw_ui(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    let block = Block::default()
        .title(" lean-tui [j/k: navigate, q: quit] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let content: Text = if state.connected {
        let mut lines: Vec<Line> = vec![
            Line::from(format!(
                "File: {}  Pos: {}:{}  ({})",
                state.cursor.filename(),
                state.cursor.line() + 1,
                state.cursor.character() + 1,
                state.cursor.method
            )),
            Line::from(""),
        ];

        if let Some(error) = &state.error {
            lines.push(Line::from(format!("Error: {error}")).style(Style::default().fg(Color::Red)));
            lines.push(Line::from(""));
        }

        if state.goals.is_empty() {
            lines.push(Line::from("No goals"));
        } else {
            let selection = state.current_selection();
            for (goal_idx, goal) in state.goals.iter().enumerate() {
                lines.push(Line::from(format!("Goal {}:", goal_idx + 1)).style(Style::default().fg(Color::Yellow)));
                
                for (hyp_idx, hyp) in goal.hyps.iter().enumerate() {
                    let names = hyp.names.join(", ");
                    let text = format!("  {} : {}", names, hyp.type_);
                    let is_selected = selection == Some(SelectableItem::Hypothesis { goal_idx, hyp_idx });
                    let style = if is_selected {
                        Style::default().bg(Color::DarkGray).fg(Color::White)
                    } else {
                        Style::default()
                    };
                    lines.push(Line::from(text).style(style));
                }
                
                let target_text = format!("  ⊢ {}", goal.target);
                let is_target_selected = selection == Some(SelectableItem::GoalTarget { goal_idx });
                let target_style = if is_target_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                lines.push(Line::from(target_text).style(target_style));
                lines.push(Line::from(""));
            }
        }

        Text::from(lines)
    } else {
        Text::from(format!("Connecting to {SOCKET_PATH}..."))
    };

    let paragraph = Paragraph::new(content)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, area);
}
