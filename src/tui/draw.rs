use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::AppState;
use crate::tui_ipc::SOCKET_PATH;

pub fn draw_ui(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    let block = Block::default()
        .title(" lean-tui ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let content = if state.connected {
        let mut lines = vec![
            format!(
                "File: {}  Pos: {}:{}  ({})",
                state.cursor.filename(),
                state.cursor.line() + 1,
                state.cursor.character() + 1,
                state.cursor.method
            ),
            String::new(),
        ];

        if let Some(error) = &state.error {
            lines.push(format!("Error: {error}"));
            lines.push(String::new());
        }

        if state.goals.is_empty() {
            lines.push("No goals".to_string());
        } else {
            for (i, goal) in state.goals.iter().enumerate() {
                lines.push(format!("Goal {}:", i + 1));
                for hyp in &goal.hyps {
                    let names = hyp.names.join(", ");
                    lines.push(format!("  {} : {}", names, hyp.type_));
                }
                lines.push(format!("  ⊢ {}", goal.target));
                lines.push(String::new());
            }
        }

        lines.join("\n")
    } else {
        format!("Connecting to {SOCKET_PATH}...")
    };

    let paragraph = Paragraph::new(content)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, area);
}
