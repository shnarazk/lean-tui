//! Shared layout constants for consistent sizing across components.

/// Layout constants for TUI component sizing.
pub struct LayoutMetrics;

impl LayoutMetrics {
    /// Height of single hypothesis line.
    pub const HYP_LINE_HEIGHT: u16 = 1;

    /// Calculate scroll position for selected item (for scrollbar state).
    #[must_use]
    pub const fn scroll_position(selected_index: usize) -> usize {
        selected_index * Self::HYP_LINE_HEIGHT as usize
    }
}
