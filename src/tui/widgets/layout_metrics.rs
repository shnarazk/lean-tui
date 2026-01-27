//! Shared layout constants for consistent sizing across components.

/// Lay out constants for TUI component sizing.
pub struct LayoutMetrics;

impl LayoutMetrics {
    /// Height of goal target display (border + content + border).
    pub const TARGET_HEIGHT: u16 = 3;

    /// Height of single hypothesis line.
    pub const HYP_LINE_HEIGHT: u16 = 1;

    /// Border overhead for hypothesis table (top border only).
    pub const HYP_BORDER_HEIGHT: u16 = 1;

    /// Calculate total goal box height for a given number of visible
    /// hypotheses.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn goal_box_height(visible_hyps: usize) -> u16 {
        let hyp_height =
            Self::HYP_BORDER_HEIGHT + (visible_hyps.max(1) as u16 * Self::HYP_LINE_HEIGHT);
        hyp_height + Self::TARGET_HEIGHT
    }

    /// Calculate scroll position for selected item (for scrollbar state).
    #[must_use]
    pub const fn scroll_position(selected_index: usize) -> usize {
        selected_index * Self::HYP_LINE_HEIGHT as usize
    }
}
