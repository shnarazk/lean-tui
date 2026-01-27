//! Goals section - the bottom portion of the Paperproof view.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    lean_rpc::Goal,
    tui::components::{
        diff_text::{diff_style, DiffState, TaggedTextExt},
        ClickRegion, SelectableItem,
    },
};

#[derive(Default)]
pub struct GoalSection;

impl GoalSection {
    #[allow(clippy::cast_possible_truncation, clippy::unused_self)]
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        goals: &[Goal],
        selection: Option<SelectableItem>,
        click_regions: &mut Vec<ClickRegion>,
    ) {
        let block = Block::default()
            .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::new().fg(Color::DarkGray))
            .title(" Goals ")
            .title_style(Style::new().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if goals.is_empty() {
            frame.render_widget(
                Paragraph::new("✓ All goals completed!")
                    .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
                inner,
            );
            return;
        }

        let lines: Vec<Line<'static>> = goals
            .iter()
            .enumerate()
            .map(|(goal_idx, goal)| {
                let is_selected = selection == Some(SelectableItem::GoalTarget { goal_idx });

                let y = inner.y + goal_idx as u16;
                if y < inner.y + inner.height {
                    click_regions.push(ClickRegion {
                        area: Rect::new(inner.x, y, inner.width, 1),
                        item: SelectableItem::GoalTarget { goal_idx },
                    });
                }

                render_goal_line(goal, is_selected, goal_idx, goals.len())
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), inner);
    }
}

fn render_goal_line(goal: &Goal, is_selected: bool, idx: usize, total: usize) -> Line<'static> {
    let state = DiffState {
        is_inserted: goal.is_inserted,
        is_removed: goal.is_removed,
        has_diff: goal.target.has_any_diff(),
    };
    let diff = diff_style(&state, is_selected, Color::Cyan);

    let marker = match (goal.is_inserted, goal.is_removed, goal.target.has_any_diff()) {
        (true, _, _) => Span::styled("[+]", Style::new().fg(Color::Green)),
        (_, true, _) => Span::styled("[-]", Style::new().fg(Color::Red)),
        (_, _, true) => Span::styled("[~]", Style::new().fg(Color::Yellow)),
        _ => Span::styled("   ", Style::new().fg(Color::DarkGray)),
    };

    let selection = if is_selected { "▶ " } else { "  " };
    let goal_num = if total > 1 { format!("[{}/{}] ", idx + 1, total) } else { String::new() };
    let case_label = goal.user_name.as_ref().map_or(String::new(), |n| format!("{n}: "));

    let mut spans = vec![
        marker,
        Span::styled(selection.to_string(), diff.style),
        Span::styled(goal_num, Style::new().fg(Color::DarkGray)),
        Span::styled(case_label, Style::new().fg(Color::Magenta)),
        Span::styled(goal.prefix.clone(), diff.style),
    ];
    spans.extend(goal.target.to_spans(diff.style));
    Line::from(spans)
}
