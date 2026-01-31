//! Diff-aware text rendering.

use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::lean_rpc::{DiffTag, TaggedText};

pub const fn diff_tag_style(tag: DiffTag, base_style: Style) -> Style {
    match tag {
        DiffTag::WasChanged | DiffTag::WillChange => base_style.fg(Color::Yellow),
        DiffTag::WasInserted | DiffTag::WillInsert => base_style.fg(Color::Green),
        DiffTag::WasDeleted | DiffTag::WillDelete => {
            base_style.fg(Color::Red).add_modifier(Modifier::DIM)
        }
    }
}

pub const fn item_style(is_selected: bool, fg_color: Color) -> Style {
    if is_selected {
        Style::new().fg(fg_color).add_modifier(Modifier::UNDERLINED)
    } else {
        Style::new().fg(fg_color)
    }
}

pub struct DiffState {
    pub is_inserted: bool,
    pub is_removed: bool,
}

pub struct DiffStyle {
    pub style: Style,
}

pub const fn diff_style(state: &DiffState, is_selected: bool, base_color: Color) -> DiffStyle {
    if state.is_inserted {
        DiffStyle {
            style: item_style(is_selected, Color::Green),
        }
    } else if state.is_removed {
        DiffStyle {
            style: item_style(is_selected, Color::Red).add_modifier(Modifier::CROSSED_OUT),
        }
    } else  {
        DiffStyle {
            style: item_style(is_selected, base_color),
        }
    } }

pub trait TaggedTextExt {
    fn to_spans(&self, base_style: Style) -> Vec<Span<'static>>;
}

impl TaggedTextExt for TaggedText {
    fn to_spans(&self, base_style: Style) -> Vec<Span<'static>> {
        match self {
            Self::Text { text } => vec![Span::styled(text.clone(), base_style)],
            Self::Tag { info, content } => {
                let style = info
                    .diff_status
                    .map_or(base_style, |tag| diff_tag_style(tag, base_style));
                content.to_spans(style)
            }
            Self::Append { items } => items
                .iter()
                .flat_map(|item| item.to_spans(base_style))
                .collect(),
        }
    }
}
