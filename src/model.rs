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

/// Clamp a buffer window `[i - buffer, i + buffer]` into `[range_start, range_end]`
/// inclusive on both ends. Returns `None` if the range is inverted or the
/// buffered window falls entirely outside it.
pub fn buffer_range_within(
    frame_index: usize,
    buffer: usize,
    range_start: usize,
    range_end: usize,
) -> Option<(usize, usize)> {
    if range_end < range_start {
        return None;
    }
    let lo = frame_index.saturating_sub(buffer).max(range_start);
    let hi = frame_index.saturating_add(buffer).min(range_end);
    if lo > hi {
        return None;
    }
    Some((lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_range_within_trims_to_range() {
        // Frame 40 with buffer 5, but the trim range is [30, 45].
        assert_eq!(buffer_range_within(40, 5, 30, 45), Some((35, 45)));
    }

    #[test]
    fn buffer_range_within_clamps_low_by_range_start() {
        // Frame 32 with buffer 5 would want [27, 37]; trim start 30 lifts the lo.
        assert_eq!(buffer_range_within(32, 5, 30, 45), Some((30, 37)));
    }

    #[test]
    fn buffer_range_within_clamps_high_by_range_end() {
        assert_eq!(buffer_range_within(43, 5, 30, 45), Some((38, 45)));
    }

    #[test]
    fn buffer_range_within_inverted_range_is_none() {
        assert_eq!(buffer_range_within(5, 2, 9, 3), None);
    }

    #[test]
    fn buffer_range_within_frame_past_end_returns_none() {
        // Frame well past the range end with a small buffer — no overlap.
        assert_eq!(buffer_range_within(50, 3, 30, 45), None);
    }

    #[test]
    fn buffer_range_within_frame_before_start_returns_none() {
        assert_eq!(buffer_range_within(5, 3, 30, 45), None);
    }

    #[test]
    fn buffer_range_within_single_frame_range() {
        assert_eq!(buffer_range_within(7, 5, 7, 7), Some((7, 7)));
    }
}
