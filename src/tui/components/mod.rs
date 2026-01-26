//! Component-based UI architecture.
//!
//! Each component encapsulates its own state, event handling, and rendering.
//! Click regions are computed during rendering to ensure consistency.

mod diff_text;
mod goal_state;
mod goal_tree;
mod goal_view;
mod header;
mod help_menu;
mod status_bar;

use crossterm::event::Event;
pub use goal_view::GoalView;
pub use header::Header;
pub use help_menu::HelpMenu;
use ratatui::{layout::Rect, Frame};
pub use status_bar::StatusBar;

use super::app::ClickRegion;

/// A UI component with co-located state, rendering, and event handling.
pub trait Component {
    /// Handle a terminal event. Returns true if the event was consumed.
    fn handle_event(&mut self, event: &Event) -> bool;

    /// Render the component and compute click regions.
    fn render(&mut self, frame: &mut Frame, area: Rect);

    /// Get the click regions computed during the last render.
    fn click_regions(&self) -> &[ClickRegion];
}
