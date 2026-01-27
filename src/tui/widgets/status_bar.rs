//! Status bar with keybindings and filter status.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::{FilterToggle, HypothesisFilters};
use crate::tui::widgets::interactive_widget::InteractiveWidget;

/// Input for the status bar.
pub struct StatusBarInput {
    pub filters: HypothesisFilters,
    pub keybindings: &'static [(&'static str, &'static str)],
    pub supported_filters: &'static [FilterToggle],
}

#[derive(Default)]
pub struct StatusBar {
    filters: HypothesisFilters,
    keybindings: &'static [(&'static str, &'static str)],
    supported_filters: &'static [FilterToggle],
}

impl InteractiveWidget for StatusBar {
    type Input = StatusBarInput;
    type Event = ();

    fn update(&mut self, input: Self::Input) {
        self.filters = input.filters;
        self.keybindings = input.keybindings;
        self.supported_filters = input.supported_filters;
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        const GLOBAL_KEYBINDINGS: &[(&str, &str)] = &[
            ("?", "help"),
            ("j/k", "nav"),
            ("[/]", "mode"),
            ("q", "quit"),
        ];

        let separator = Span::raw(" │ ");

        // Global keybindings
        let global_spans = GLOBAL_KEYBINDINGS
            .iter()
            .enumerate()
            .flat_map(|(i, (key, desc))| {
                let prefix = (i > 0).then(|| separator.clone());
                prefix.into_iter().chain([
                    Span::styled(*key, Style::new().fg(Color::Cyan)),
                    Span::raw(format!(": {desc}")),
                ])
            });

        // Mode-specific keybindings
        let mode_spans = self.keybindings.iter().flat_map(|(key, desc)| {
            [
                separator.clone(),
                Span::styled(*key, Style::new().fg(Color::Yellow)),
                Span::raw(format!(": {desc}")),
            ]
        });

        // Navigation shortcuts (d/t) in magenta, before filters
        let nav_spans = [
            separator.clone(),
            Span::styled("d", Style::new().fg(Color::Magenta)),
            Span::raw(": def"),
            Span::raw(" "),
            Span::styled("t", Style::new().fg(Color::Magenta)),
            Span::raw(": type"),
        ];

        let filter_status = build_filter_status(self.filters, self.supported_filters);
        let filter_span = (!filter_status.is_empty())
            .then(|| Span::styled(format!(" [{filter_status}]"), Style::new().fg(Color::Green)));

        let spans: Vec<Span> = global_spans
            .chain(mode_spans)
            .chain(nav_spans)
            .chain(filter_span)
            .collect();
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

fn build_filter_status(filters: HypothesisFilters, supported: &[FilterToggle]) -> String {
    [
        (FilterToggle::Instances, filters.hide_instances, 'i'),
        (FilterToggle::Inaccessible, filters.hide_inaccessible, 'a'),
        (FilterToggle::LetValues, filters.hide_let_values, 'l'),
        (FilterToggle::ReverseOrder, filters.reverse_order, 'r'),
    ]
    .into_iter()
    .filter(|(toggle, _, _)| supported.contains(toggle))
    .filter_map(|(_, enabled, c)| enabled.then_some(c))
    .collect()
}
