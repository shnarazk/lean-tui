//! Diff-aware text rendering utilities.
//!
//! Provides rendering of TaggedText and hypotheses with diff highlighting.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::{
    lean_rpc::{DiffTag, Goal, Hypothesis, TaggedText},
    tui::app::HypothesisFilters,
};

/// Convert a DiffTag to style modifiers.
pub const fn diff_tag_style(tag: DiffTag, base_style: Style) -> Style {
    match tag {
        DiffTag::WasChanged | DiffTag::WillChange => base_style.fg(Color::Yellow),
        DiffTag::WasInserted | DiffTag::WillInsert => base_style.fg(Color::Green),
        DiffTag::WasDeleted | DiffTag::WillDelete => {
            base_style.fg(Color::Red).add_modifier(Modifier::DIM)
        }
    }
}

/// Style for items with optional selection highlighting.
pub const fn item_style(is_selected: bool, fg_color: Color) -> Style {
    if is_selected {
        Style::new().bg(Color::DarkGray).fg(fg_color)
    } else {
        Style::new().fg(fg_color)
    }
}

/// Selection indicator prefix.
pub const fn selection_prefix(is_selected: bool) -> &'static str {
    if is_selected {
        "▶ "
    } else {
        "  "
    }
}

/// Compute style and diff marker based on insertion/removal/change state.
pub fn diff_style(
    is_inserted: bool,
    is_removed: bool,
    has_diff: bool,
    is_selected: bool,
    base_color: Color,
) -> (Style, &'static str) {
    if is_inserted {
        (item_style(is_selected, Color::Green), " [+]")
    } else if is_removed {
        (
            item_style(is_selected, Color::Red).add_modifier(Modifier::CROSSED_OUT),
            " [-]",
        )
    } else if has_diff {
        (item_style(is_selected, base_color), " [~]")
    } else {
        (item_style(is_selected, base_color), "")
    }
}

/// Extension trait for TaggedText to convert to ratatui spans.
pub trait TaggedTextExt {
    /// Convert to ratatui spans with per-subexpression diff highlighting.
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

/// Render a hypothesis line with diff styling.
pub fn render_hypothesis_line(
    hyp: &Hypothesis,
    is_selected: bool,
    filters: HypothesisFilters,
) -> Line<'static> {
    let names = hyp.names.join(", ");
    let prefix = selection_prefix(is_selected);
    let (base_style, diff_marker) = diff_style(
        hyp.is_inserted,
        hyp.is_removed,
        hyp.type_.has_any_diff(),
        is_selected,
        Color::White,
    );

    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(prefix.to_string(), base_style),
        Span::styled(format!("{names} : "), base_style),
    ];
    spans.extend(hyp.type_.to_spans(base_style));

    if !filters.hide_let_values {
        if let Some(ref val) = hyp.val {
            spans.push(Span::styled(" := ".to_string(), base_style));
            spans.extend(val.to_spans(base_style));
        }
    }

    if !diff_marker.is_empty() {
        spans.push(Span::styled(diff_marker.to_string(), base_style));
    }

    Line::from(spans)
}

/// Render a goal target line with diff styling.
pub fn render_target_line(goal: &Goal, is_selected: bool) -> Line<'static> {
    let prefix = selection_prefix(is_selected);
    let (base_style, diff_marker) = diff_style(
        goal.is_inserted,
        goal.is_removed,
        goal.target.has_any_diff(),
        is_selected,
        Color::Cyan,
    );

    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(prefix.to_string(), base_style),
        Span::styled(goal.prefix.clone(), base_style),
    ];
    spans.extend(goal.target.to_spans(base_style));

    if !diff_marker.is_empty() {
        spans.push(Span::styled(diff_marker.to_string(), base_style));
    }

    Line::from(spans)
}
