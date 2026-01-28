//! Virtual canvas for scrollable content with viewport clipping.

use ratatui::layout::Rect;

/// Virtual canvas state for scrollable content.
#[derive(Debug, Default, Clone, Copy)]
pub struct VirtualCanvas {
    pub content_width: i32,
    pub content_height: i32,
    pub scroll_x: i32,
    pub scroll_y: i32,
}

impl VirtualCanvas {
    pub const fn new(content_width: i32, content_height: i32) -> Self {
        Self {
            content_width,
            content_height,
            scroll_x: 0,
            scroll_y: 0,
        }
    }

    /// Center the viewport on a target position.
    pub fn scroll_to_center(
        &mut self,
        target_x: i32,
        target_y: i32,
        target_w: u16,
        target_h: u16,
        viewport: Rect,
    ) {
        let center_x = target_x + i32::from(target_w) / 2;
        let center_y = target_y + i32::from(target_h) / 2;

        let max_x = (self.content_width - i32::from(viewport.width)).max(0);
        let max_y = (self.content_height - i32::from(viewport.height)).max(0);

        self.scroll_x = (center_x - i32::from(viewport.width) / 2).clamp(0, max_x);
        self.scroll_y = (center_y - i32::from(viewport.height) / 2).clamp(0, max_y);
    }

    pub fn needs_horizontal_scroll(&self, viewport: Rect) -> bool {
        self.content_width > i32::from(viewport.width)
    }

    pub fn needs_vertical_scroll(&self, viewport: Rect) -> bool {
        self.content_height > i32::from(viewport.height)
    }

    /// Clip a virtual rectangle to the viewport. Returns None if not visible.
    #[allow(clippy::cast_sign_loss)]
    pub fn clip_to_viewport(
        &self,
        x: i32,
        y: i32,
        width: u16,
        height: u16,
        viewport: Rect,
    ) -> Option<Rect> {
        let rel_x = x - self.scroll_x;
        let rel_y = y - self.scroll_y;

        // Outside viewport?
        if rel_x >= i32::from(viewport.width)
            || rel_y >= i32::from(viewport.height)
            || rel_x + i32::from(width) <= 0
            || rel_y + i32::from(height) <= 0
        {
            return None;
        }

        let (rx, rw) = clip_axis(rel_x, width, viewport.x, viewport.width);
        let (ry, rh) = clip_axis(rel_y, height, viewport.y, viewport.height);

        (rw > 0 && rh > 0).then_some(Rect::new(rx, ry, rw, rh))
    }
}

#[allow(clippy::cast_sign_loss)]
fn clip_axis(rel_pos: i32, size: u16, origin: u16, area_size: u16) -> (u16, u16) {
    if rel_pos < 0 {
        let clip = (-rel_pos) as u16;
        (origin, size.saturating_sub(clip).min(area_size))
    } else {
        let pos = rel_pos as u16;
        (origin + pos, size.min(area_size.saturating_sub(pos)))
    }
}
