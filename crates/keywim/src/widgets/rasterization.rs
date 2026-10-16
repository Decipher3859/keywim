//! Converts widget cells into pixels using a parsed font.
use super::color_palette::color_rgb;
use fontdue::{Font, FontSettings};
use ratatui::buffer::Buffer;
use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::Transform,
};

/// Pixel output with premultiplied RGBA channels, stored row by row.
pub struct RasterizedWidget {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl RasterizedWidget {
    pub fn new(width: usize, height: usize) -> Result<Self, &'static str> {
        // Each RGBA pixel need four bytes; start with the transparent pixels.
        let byte_count = width
            .checked_mul(height)
            .and_then(|count| count.checked_mul(4))
            .ok_or("Pixel buffer dimensions are to large")?;

        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(byte_count)
            .map_err(|_| "Could not allocate pixel buffer")?;
        pixels.resize(byte_count, 0);

        Ok(Self {
            width,
            height,
            pixels,
        })
    }
    pub fn into_render_buffer(self) -> Result<MemoryRenderBuffer, &'static str> {
        let width = i32::try_from(self.width).map_err(|_| "Image to wide")?;
        let height = i32::try_from(self.height).map_err(|_| "Image to tall")?;
        if width == 0 || height == 0 {
            return Err("Image dimensions must be positive");
        }

        // On little-endian systems, ABGR8888 stores bytes in RGBA order.
        Ok(MemoryRenderBuffer::from_slice(
            &self.pixels,
            Fourcc::Abgr8888,
            (width, height),
            1,
            Transform::Normal,
            None,
        ))
    }

    fn blend_pixel(&mut self, x: usize, y: usize, rgb: [u8; 3], alpha: u8) {
        // Clip pixels outside the image, including overhanging glyph edges.
        if x >= self.width || y >= self.height {
            return;
        }

        let offset = (y * self.width + x) * 4;
        let target = &mut self.pixels[offset..offset + 4];
        let alpha = u32::from(alpha);
        let inverse = 255 - alpha;

        // Composite over existing pixels while keeping RGB premultiplied.
        for channel in 0..3 {
            target[channel] =
                ((u32::from(rgb[channel]) * alpha + u32::from(target[channel]) * inverse + 127)
                    / 255) as u8;
        }
        target[3] = (alpha + (u32::from(target[3]) * inverse + 127) / 255) as u8;
    }

    fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, rgb: [u8; 3]) {
        // Clip the cell background to the image bounds.
        let right = x.saturating_add(width).min(self.width);
        let bottom = y.saturating_add(height).min(self.height);

        for py in y..bottom {
            for px in x..right {
                self.blend_pixel(px, py, rgb, 255);
            }
        }
    }
}

pub struct WidgetRasterizer {
    font: Font,
    font_size: f32,
    cell_width: usize,
    cell_height: usize,
    baseline: i32,
}

impl WidgetRasterizer {
    pub fn new(font_data: &[u8], font_size: f32) -> Result<Self, &'static str> {
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err("Font size must be finite and positive");
        }
        // Parse once so later updates can reuse the same font.
        let font = Font::from_bytes(font_data, FontSettings::default())?;
        let lines = font
            .horizontal_line_metrics(font_size)
            .ok_or("Font has no horizontal line metrics")?;

        // Cells gain pixel values to render them outside of a terminal
        let cell_width = font.metrics('M', font_size).advance_width.ceil().max(1.0) as usize;
        let baseline = lines.ascent.ceil() as i32;
        let cell_height = (lines.ascent.ceil() - lines.descent.floor()
            + lines.line_gap.max(0.0).ceil())
        .max(1.0) as usize;

        Ok(Self {
            font,
            font_size,
            cell_width,
            cell_height,
            baseline,
        })
    }

    pub fn pixel_size(&self, columns: u16, rows: u16) -> Option<(usize, usize)> {
        // Check multiplication before using grid dimensions for allocation.
        let width = self.cell_width.checked_mul(usize::from(columns))?;
        let height = self.cell_height.checked_mul(usize::from(rows))?;
        Some((width, height))
    }

    fn draw_glyph(
        &self,
        image: &mut RasterizedWidget,
        character: char,
        origin: (usize, usize),
        rgb: [u8; 3],
    ) {
        let (metrics, coverage) = self.font.rasterize(character, self.font_size);
        // Convert the font's upward Y axis to image rows below the baseline.
        let left = origin.0 as i64 + i64::from(metrics.xmin);
        let top = origin.1 as i64 + i64::from(self.baseline)
            - i64::from(metrics.ymin)
            - metrics.height as i64;

        for y in 0..metrics.height {
            for x in 0..metrics.width {
                let px = left + x as i64;
                let py = top + y as i64;
                if px >= 0 && py >= 0 {
                    image.blend_pixel(
                        px as usize,
                        py as usize,
                        rgb,
                        coverage[y * metrics.width + x],
                    );
                }
            }
        }
    }

    pub fn rasterize(&self, buffer: &Buffer) -> Result<RasterizedWidget, &'static str> {
        let area = buffer.area;
        let (width, height) = self
            .pixel_size(area.width, area.height)
            .ok_or("Widget dimensions are too large")?;
        let mut image = RasterizedWidget::new(width, height)?;

        // Paint all backgrounds before glyphs so adjacent cells cannot erase them.
        for row in 0..area.height {
            for column in 0..area.width {
                let cell = &buffer[(area.x + column, area.y + row)];
                image.fill_rect(
                    usize::from(column) * self.cell_width,
                    usize::from(row) * self.cell_height,
                    self.cell_width,
                    self.cell_height,
                    color_rgb(cell.bg, [0, 0, 0]),
                );
            }
        }

        // Draw glyphs over the completed background layer.
        for row in 0..area.height {
            for column in 0..area.width {
                let cell = &buffer[(area.x + column, area.y + row)];
                let mut characters = cell.symbol().chars();
                let Some(character) = characters.next() else {
                    continue;
                };
                // Complex graphemes require shaping rather than separate glyphs.
                if characters.next().is_some() {
                    return Err("Multi-character cell symbols are not supported yet");
                }

                self.draw_glyph(
                    &mut image,
                    character,
                    (
                        usize::from(column) * self.cell_width,
                        usize::from(row) * self.cell_height,
                    ),
                    color_rgb(cell.fg, [255, 255, 255]),
                );
            }
        }
        Ok(image)
    }
}
