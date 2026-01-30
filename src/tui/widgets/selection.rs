//! Reusable selection state and click region handling.

use ratatui::layout::Rect;

/// Unified selection type for all display modes.
/// All selections reference data in `ProofDag`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// Initial hypothesis from theorem statement.
    InitialHyp { hyp_idx: usize },
    /// Hypothesis at a proof step (`node_id` indexes into `ProofDag`).
    Hyp { node_id: u32, hyp_idx: usize },
    /// Goal at a proof step.
    Goal { node_id: u32, goal_idx: usize },
    /// The theorem conclusion.
    Theorem,
}

/// A clickable region mapped to a selection.
#[derive(Debug, Clone)]
pub struct ClickRegion {
    pub area: Rect,
    pub selection: Selection,
}

/// Manages selection state for navigable items.
#[derive(Debug, Default)]
pub struct SelectionState {
    selected_index: Option<usize>,
    click_regions: Vec<ClickRegion>,
}

impl SelectionState {
    /// Clear click regions (call at start of render).
    pub fn clear_regions(&mut self) {
        self.click_regions.clear();
    }

    /// Add a click region.
    pub fn add_region(&mut self, area: Rect, selection: Selection) {
        self.click_regions.push(ClickRegion { area, selection });
    }

    /// Reset selection to first item if items exist.
    pub fn reset(&mut self, item_count: usize) {
        self.selected_index = (item_count > 0).then_some(0);
    }

    /// Move selection to previous item.
    pub fn select_previous(&mut self, item_count: usize) {
        if item_count == 0 {
            return;
        }
        self.selected_index = Some(self.selected_index.map_or(0, |i| i.saturating_sub(1)));
    }

    /// Move selection to next item.
    pub const fn select_next(&mut self, item_count: usize) {
        if item_count == 0 {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) if i < item_count - 1 => i + 1,
            Some(i) => i,
            None => 0,
        });
    }

    /// Get currently selected item from a list.
    pub fn current_selection<'a>(&self, items: &'a [Selection]) -> Option<&'a Selection> {
        self.selected_index.and_then(|i| items.get(i))
    }

    /// Handle a click at (x, y). Returns true if selection changed.
    pub fn handle_click(&mut self, x: u16, y: u16, items: &[Selection]) -> bool {
        let Some(region) = self.find_click_region(x, y) else {
            return false;
        };

        if let Some(idx) = items.iter().position(|i| *i == region.selection) {
            self.selected_index = Some(idx);
            return true;
        }
        false
    }

    fn find_click_region(&self, x: u16, y: u16) -> Option<&ClickRegion> {
        self.click_regions.iter().find(|region| {
            region.area.x <= x
                && x < region.area.x + region.area.width
                && region.area.y <= y
                && y < region.area.y + region.area.height
        })
    }
}
