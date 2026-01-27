use ratatui::{layout::Rect, Frame};

pub trait InteractiveWidget {
    type Input;
    /// Use `()` for noninteractive components.
    type Event;

    fn update(&mut self, input: Self::Input);

    fn handle_event(&mut self, _event: Self::Event) -> bool {
        false
    }

    fn render(&mut self, frame: &mut Frame, area: Rect);
}
