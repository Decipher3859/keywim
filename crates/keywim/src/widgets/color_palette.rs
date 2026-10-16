//! Base colors used when rasterizing terminal-style widgets.
use ratatui::style::Color;

pub const BASE_PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],       // Black
    [128, 0, 0],     // Red
    [0, 128, 0],     // Green
    [128, 128, 0],   // Yellow
    [0, 0, 128],     // Blue
    [128, 0, 128],   // Magenta
    [0, 128, 128],   // Cyan
    [192, 192, 192], // Gray
    [128, 128, 128], // Dark gray
    [255, 0, 0],     // Bright red
    [0, 255, 0],     // Bright green
    [255, 255, 0],   // Bright yellow
    [0, 0, 255],     // Bright blue
    [255, 0, 255],   // Bright magenta
    [0, 255, 255],   // Bright cyan
    [255, 255, 255], // White
];

pub fn indexed_rgb(index: u8) -> [u8; 3] {
    match index {
        0..=15 => BASE_PALETTE[usize::from(index)],
        16..=231 => {
            // The next 216 colors form a cube with six levels per channel.
            let levels = [0, 95, 135, 175, 215, 255];
            let value = usize::from(index - 16);
            [
                levels[value / 36],
                levels[(value / 6) % 6],
                levels[value % 6],
            ]
        }
        232..=255 => {
            // The final 24 entries are evenly spaced grayscale values.
            let gray = 8 + (index - 232) * 10;
            [gray, gray, gray]
        }
    }
}

pub fn color_rgb(color: Color, default: [u8; 3]) -> [u8; 3] {
    // Reset inherits the caller's foreground or background default.
    let index = match color {
        Color::Reset => return default,
        Color::Rgb(r, g, b) => return [r, g, b],
        Color::Indexed(index) => return indexed_rgb(index),
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
    };
    BASE_PALETTE[index]
}
