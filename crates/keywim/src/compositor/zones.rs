use smithay::{
    backend::renderer::element::memory::MemoryRenderBuffer,
    utils::{Logical, Point},
};

/// Holds content for the topmost visual layer application and meta zones.
#[derive(Default)]
pub struct NotificationZone {
    pub buffer: Option<MemoryRenderBuffer>,
    pub location: Point<i32, Logical>,
}

impl NotificationZone {
    // Accept rasterized content from any source
    pub fn set_content(&mut self, buffer: MemoryRenderBuffer, location: Point<i32, Logical>) {
        self.buffer = Some(buffer);
        self.location = location;
    }

    // Remove visible content while keeping the zone available for reuse.
    pub fn clear(&mut self) {
        self.buffer = None;
    }
}
