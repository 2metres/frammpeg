use serde::{Deserialize, Serialize};

pub const DEFAULT_BUFFER: usize = 5;
pub const MAX_BUFFER: usize = 30;
pub const DEFAULT_STROKE_RGBA: [u8; 4] = [0xFF, 0x3D, 0x71, 0xFF];
pub const DEFAULT_TEXT_RGBA: [u8; 4] = [0xFF, 0xF2, 0x7A, 0xFF];
pub const DEFAULT_STROKE_WIDTH: f32 = 3.0;
pub const DEFAULT_FONT_SIZE: f32 = 24.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Annotation {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        stroke_color: [u8; 4],
        stroke_width: f32,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        font_size: f32,
        color: [u8; 4],
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Moment {
    pub frame_index: usize,
    pub buffer: usize,
    pub note: String,
}

impl Moment {
    pub fn new(frame_index: usize) -> Self {
        Self {
            frame_index,
            buffer: DEFAULT_BUFFER,
            note: String::new(),
        }
    }
}

/// Clamp a buffer window `[i - buffer, i + buffer]` into `[0, total)` inclusive
/// on the low end, exclusive on the high end. Returns `(lo, hi)` inclusive.
pub fn buffer_range(frame_index: usize, buffer: usize, total: usize) -> Option<(usize, usize)> {
    if total == 0 {
        return None;
    }
    let last = total - 1;
    let lo = frame_index.saturating_sub(buffer);
    let hi = frame_index.saturating_add(buffer).min(last);
    Some((lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_range_typical() {
        assert_eq!(buffer_range(40, 5, 100), Some((35, 45)));
    }

    #[test]
    fn buffer_range_clamps_low() {
        assert_eq!(buffer_range(2, 5, 100), Some((0, 7)));
    }

    #[test]
    fn buffer_range_clamps_high() {
        assert_eq!(buffer_range(97, 5, 100), Some((92, 99)));
    }

    #[test]
    fn buffer_range_zero_total() {
        assert_eq!(buffer_range(0, 5, 0), None);
    }

    #[test]
    fn buffer_range_zero_buffer() {
        assert_eq!(buffer_range(10, 0, 100), Some((10, 10)));
    }

    #[test]
    fn buffer_range_full_span() {
        assert_eq!(buffer_range(0, 100, 5), Some((0, 4)));
    }
}
