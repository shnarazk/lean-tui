//! Theorem pane - displays the theorem being proved in the semantic tableau.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget, Wrap},
};

use super::{ClickRegion, Selection};
use crate::tui::widgets::theme::Theme;

/// State for the theorem pane.
#[derive(Default)]
pub struct TheoremPaneState {
    /// Click regions from last render.
    pub click_regions: Vec<ClickRegion>,
}

impl TheoremPaneState {
    /// Find click at position.
    pub fn find_click_at(&self, x: u16, y: u16) -> Option<Selection> {
        self.click_regions
            .iter()
            .find(|r| {
                x >= r.area.x
                    && x < r.area.x + r.area.width
                    && y >= r.area.y
                    && y < r.area.y + r.area.height
            })
            .map(|r| r.selection)
    }
}

/// Theorem pane widget - displays the theorem conclusion.
pub struct TheoremPane<'a> {
    goal: &'a str,
    selection: Option<Selection>,
}

impl<'a> TheoremPane<'a> {
    pub const fn new(goal: &'a str, selection: Option<Selection>) -> Self {
        Self { goal, selection }
    }
}

impl StatefulWidget for TheoremPane<'_> {
    type State = TheoremPaneState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.click_regions.clear();

        let is_selected = matches!(self.selection, Some(Selection::Theorem));

        state.click_regions.push(ClickRegion {
            area,
            selection: Selection::Theorem,
        });

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(Color::Magenta))
            .title(Span::styled(
                " THEOREM ",
                Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        let mut style = Style::new().fg(Theme::GOAL_FG);
        if is_selected {
            style = style.add_modifier(Modifier::UNDERLINED);
        }

        Paragraph::new(Line::from(vec![Span::styled(
            format!("⊢ {}", self.goal),
            style,
        )]))
        .wrap(Wrap { trim: true })
        .render(inner, buf);
    }
}
