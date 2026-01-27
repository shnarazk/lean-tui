//! Interactive widget traits extending ratatui's `StatefulWidget`.

use ratatui::{layout::Rect, widgets::StatefulWidget, Frame};

/// Extension trait for `StatefulWidget` that adds state management and event
/// handling.
///
/// This trait extends ratatui's `StatefulWidget` with methods for updating
/// state and handling events, providing a complete interactive widget pattern.
///
/// # Example
///
/// ```ignore
/// struct MyWidgetState { value: i32 }
/// struct MyWidget;
///
/// impl StatefulWidget for MyWidget {
///     type State = MyWidgetState;
///     fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) { /* ... */ }
/// }
///
/// impl InteractiveStatefulWidget for MyWidget {
///     type Input = i32;
///     type Event = KeyEvent;
///
///     fn update_state(state: &mut Self::State, input: Self::Input) {
///         state.value = input;
///     }
///
///     fn handle_event(state: &mut Self::State, event: Self::Event) -> bool {
///         // handle event, return true if consumed
///         false
///     }
/// }
/// ```
pub trait InteractiveStatefulWidget: StatefulWidget {
    /// Input data type for updating state.
    type Input;
    /// Event type for handling (use `()` for non-interactive widgets).
    type Event;

    /// Update the widget's state with new input data.
    fn update_state(state: &mut Self::State, input: Self::Input);

    /// Handle an event. Returns true if the event was consumed.
    fn handle_event(_state: &mut Self::State, _event: Self::Event) -> bool {
        false
    }
}

/// Trait for self-contained interactive components that render directly to
/// Frame.
///
/// Use this for higher-level app components like display modes that manage
/// their own state and render using Frame (not Buffer).
pub trait InteractiveComponent {
    /// Input data type for updating state.
    type Input;
    /// Event type for handling (use `()` for non-interactive components).
    type Event;

    /// Update state with new input data.
    fn update(&mut self, input: Self::Input);

    /// Handle an event. Returns true if the event was consumed.
    fn handle_event(&mut self, _event: Self::Event) -> bool {
        false
    }

    /// Render the component to a Frame.
    fn render(&mut self, frame: &mut Frame, area: Rect);
}
