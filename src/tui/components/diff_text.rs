//! Diff-aware text rendering.

use std::iter;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::HypothesisFilters;
use crate::lean_rpc::{DiffTag, Goal, Hypothesis, TaggedText};

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
        Style::new().bg(Color::DarkGray).fg(fg_color)
    } else {
        Style::new().fg(fg_color)
    }
}

pub const fn selection_prefix(is_selected: bool) -> &'static str {
    if is_selected {
        "▶ "
    } else {
        "  "
    }
}

pub struct DiffState {
    pub is_inserted: bool,
    pub is_removed: bool,
    pub has_diff: bool,
}

pub const fn diff_style(
    state: &DiffState,
    is_selected: bool,
    base_color: Color,
) -> (Style, &'static str) {
    if state.is_inserted {
        (item_style(is_selected, Color::Green), " [+]")
    } else if state.is_removed {
        (
            item_style(is_selected, Color::Red).add_modifier(Modifier::CROSSED_OUT),
            " [-]",
        )
    } else if state.has_diff {
        (item_style(is_selected, base_color), " [~]")
    } else {
        (item_style(is_selected, base_color), "")
    }
}

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

pub fn render_hypothesis_line(
    hyp: &Hypothesis,
    is_selected: bool,
    filters: HypothesisFilters,
) -> Line<'static> {
    let state = DiffState {
        is_inserted: hyp.is_inserted,
        is_removed: hyp.is_removed,
        has_diff: hyp.type_.has_any_diff(),
    };
    let (style, marker) = diff_style(&state, is_selected, Color::White);

    let names = hyp.names.join(", ");
    let prefix = selection_prefix(is_selected);

    let base_spans = [
        Span::styled(prefix.to_string(), style),
        Span::styled(format!("{names} : "), style),
    ];

    let type_spans = hyp.type_.to_spans(style);

    let value_spans: Vec<Span<'static>> = match (&hyp.val, filters.hide_let_values) {
        (Some(val), false) => iter::once(Span::styled(" := ".to_string(), style))
            .chain(val.to_spans(style))
            .collect(),
        _ => Vec::new(),
    };

    let marker_span = (!marker.is_empty()).then(|| Span::styled(marker.to_string(), style));

    Line::from(
        base_spans
            .into_iter()
            .chain(type_spans)
            .chain(value_spans)
            .chain(marker_span)
            .collect::<Vec<_>>(),
    )
}

pub fn render_target_line(goal: &Goal, is_selected: bool) -> Line<'static> {
    let state = DiffState {
        is_inserted: goal.is_inserted,
        is_removed: goal.is_removed,
        has_diff: goal.target.has_any_diff(),
    };
    let (style, marker) = diff_style(&state, is_selected, Color::Cyan);

    let prefix = selection_prefix(is_selected);

    let base_spans = [
        Span::styled(prefix.to_string(), style),
        Span::styled(goal.prefix.clone(), style),
    ];

    let target_spans = goal.target.to_spans(style);
    let marker_span = (!marker.is_empty()).then(|| Span::styled(marker.to_string(), style));

    Line::from(
        base_spans
            .into_iter()
            .chain(target_spans)
            .chain(marker_span)
            .collect::<Vec<_>>(),
    )
}
