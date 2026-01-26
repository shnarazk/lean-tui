//! Status bar with keybindings and filter status.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::{Component, HypothesisFilters};

#[derive(Default)]
pub struct StatusBar {
    filters: HypothesisFilters,
}

impl Component for StatusBar {
    type Input = HypothesisFilters;
    type Event = ();

    fn update(&mut self, input: Self::Input) {
        self.filters = input;
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        const KEYBINDINGS: &[(&str, &str)] = &[
            ("?", "help"),
            ("j/k", "nav"),
            ("Enter", "go"),
            ("q", "quit"),
        ];

        let separator = Span::raw(" │ ");
        let keybind_spans = KEYBINDINGS.iter().enumerate().flat_map(|(i, (key, desc))| {
            let prefix = (i > 0).then(|| separator.clone());
            prefix.into_iter().chain([
                Span::styled(*key, Style::new().fg(Color::Cyan)),
                Span::raw(format!(": {desc}")),
            ])
        });

        let filter_status = build_filter_status(self.filters);
        let filter_span = (!filter_status.is_empty())
            .then(|| Span::styled(format!(" [{filter_status}]"), Style::new().fg(Color::Green)));

        let spans: Vec<Span> = keybind_spans.chain(filter_span).collect();
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

fn build_filter_status(filters: HypothesisFilters) -> String {
    [
        (!filters.hide_definition, 'd'),
        (filters.hide_instances, 'i'),
        (filters.hide_types, 't'),
        (filters.hide_inaccessible, 'a'),
        (filters.hide_let_values, 'l'),
        (filters.reverse_order, 'r'),
    ]
    .into_iter()
    .filter_map(|(enabled, c)| enabled.then_some(c))
    .collect()
}
