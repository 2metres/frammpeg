use std::time::Duration;

use egui::{Button, Color32, RichText, Ui, Vec2};

use crate::theme;

const STEP_SMALL: usize = 1;
const STEP_LARGE: usize = 10;

const STEP_BUTTON_W: f32 = 34.0;
const PLAY_BUTTON_W: f32 = 48.0;
const BUTTON_GAP: f32 = 4.0;
const STEP_BUTTON_COUNT: f32 = 6.0;
const ROW_WIDTH: f32 = STEP_BUTTON_W * STEP_BUTTON_COUNT + PLAY_BUTTON_W + BUTTON_GAP * 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportAction {
    Home,
    Back(usize),
    Fwd(usize),
    End,
    TogglePlay,
}

#[derive(Debug, Clone, Copy)]
pub struct TransportView {
    pub enabled: bool,
    pub playing: bool,
}

/// Draw the centered transport button row. Returns the first pressed action.
pub fn draw(ui: &mut Ui, view: TransportView) -> Option<TransportAction> {
    let mut action: Option<TransportAction> = None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = BUTTON_GAP;
        let pad = ((ui.available_width() - ROW_WIDTH) * 0.5).max(0.0);
        ui.add_space(pad);
        if step_button(ui, "\u{23EE}", view.enabled, "First frame (Home)") {
            action = Some(TransportAction::Home);
        }
        if step_button(ui, "\u{23EA}", view.enabled, "Back 10 (Shift+Left)") {
            action = Some(TransportAction::Back(STEP_LARGE));
        }
        if step_button(ui, "\u{25C0}", view.enabled, "Back 1 (Left / ,)") {
            action = Some(TransportAction::Back(STEP_SMALL));
        }
        if play_button(ui, view) {
            action = Some(TransportAction::TogglePlay);
        }
        if step_button(ui, "\u{25B6}", view.enabled, "Forward 1 (Right / .)") {
            action = Some(TransportAction::Fwd(STEP_SMALL));
        }
        if step_button(ui, "\u{23E9}", view.enabled, "Forward 10 (Shift+Right)") {
            action = Some(TransportAction::Fwd(STEP_LARGE));
        }
        if step_button(ui, "\u{23ED}", view.enabled, "Last frame (End)") {
            action = Some(TransportAction::End);
        }
    });

    action
}

fn step_button(ui: &mut Ui, glyph: &str, enabled: bool, tooltip: &str) -> bool {
    let color = if enabled {
        Color32::WHITE
    } else {
        theme::TEXT_MUTED
    };
    let text = RichText::new(glyph).size(22.0).color(color).strong();
    let btn = Button::new(text)
        .min_size(Vec2::new(34.0, 30.0))
        .frame(false);
    ui.add_enabled(enabled, btn)
        .on_hover_text(tooltip)
        .clicked()
}

fn play_button(ui: &mut Ui, view: TransportView) -> bool {
    let glyph = if view.playing { "\u{23F8}" } else { "\u{25B6}" };
    let color = if view.enabled {
        Color32::WHITE
    } else {
        theme::TEXT_MUTED
    };
    let text = RichText::new(glyph).size(28.0).color(color).strong();
    let btn = Button::new(text)
        .min_size(Vec2::new(48.0, 30.0))
        .frame(false);
    let tooltip = if view.playing {
        "Pause (Space)"
    } else {
        "Play (Space)"
    };
    ui.add_enabled(view.enabled, btn)
        .on_hover_text(tooltip)
        .clicked()
}

/// Given a playback fps and the elapsed time since the last tick, return how
/// many whole frames to advance and how much leftover time to carry into the
/// next tick.
///
/// Fps <= 0 disables the play loop (returns `(0, elapsed)`).
pub fn advance_frames(fps: f32, elapsed: Duration) -> (u32, Duration) {
    if !fps.is_finite() || fps <= 0.0 {
        return (0, elapsed);
    }
    let period_s = 1.0 / fps as f64;
    let elapsed_s = elapsed.as_secs_f64();
    if elapsed_s < period_s {
        return (0, elapsed);
    }
    let frames = (elapsed_s / period_s).floor() as u32;
    let consumed_s = frames as f64 * period_s;
    let leftover = Duration::from_secs_f64((elapsed_s - consumed_s).max(0.0));
    (frames, leftover)
}

/// Advance the current frame by `frames`, clamping at the inclusive
/// `range_end` (either the last frame of the clip or the trim range end).
/// Returns `(next, hit_end)`; `hit_end` is true when the advance reached
/// or overshot the range end — the caller should pause playback.
pub fn step_play_to(current: usize, frames: u32, range_end: usize) -> (usize, bool) {
    let next = current.saturating_add(frames as usize);
    if next >= range_end {
        (range_end, true)
    } else {
        (next, false)
    }
}

/// One-frame tick period at the given fps. Used to schedule repaints.
pub fn frame_period(fps: f32) -> Duration {
    if !fps.is_finite() || fps <= 0.0 {
        return Duration::from_millis(33);
    }
    let s = 1.0 / fps as f64;
    Duration::from_secs_f64(s.clamp(0.001, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_frames_zero_when_below_period() {
        let (n, leftover) = advance_frames(30.0, Duration::from_millis(10));
        assert_eq!(n, 0);
        assert_eq!(leftover, Duration::from_millis(10));
    }

    #[test]
    fn advance_frames_one_frame_at_period() {
        let (n, leftover) = advance_frames(30.0, Duration::from_secs_f32(1.0 / 30.0));
        assert_eq!(n, 1);
        assert!(leftover < Duration::from_millis(1));
    }

    #[test]
    fn advance_frames_multiple_frames() {
        // 100ms at 30fps -> 3 frames (33.33ms each), ~0.33ms leftover.
        let (n, leftover) = advance_frames(30.0, Duration::from_millis(100));
        assert_eq!(n, 3);
        assert!(leftover < Duration::from_millis(2));
    }

    #[test]
    fn advance_frames_zero_fps_disables() {
        let (n, leftover) = advance_frames(0.0, Duration::from_secs(1));
        assert_eq!(n, 0);
        assert_eq!(leftover, Duration::from_secs(1));
    }

    #[test]
    fn step_play_to_clamps_at_range_end() {
        // range end = 15, current 14, +3 -> clamp to 15 and flag hit_end.
        assert_eq!(step_play_to(14, 3, 15), (15, true));
    }

    #[test]
    fn step_play_to_advances_normally_within_range() {
        assert_eq!(step_play_to(10, 2, 15), (12, false));
    }

    #[test]
    fn step_play_to_reaches_range_end_exactly() {
        assert_eq!(step_play_to(14, 1, 15), (15, true));
    }

    #[test]
    fn step_play_to_current_at_end_stays_and_flags() {
        assert_eq!(step_play_to(15, 1, 15), (15, true));
    }

    #[test]
    fn frame_period_reasonable_at_common_fps() {
        let p24 = frame_period(24.0);
        assert!(p24 > Duration::from_millis(40) && p24 < Duration::from_millis(45));
        let p60 = frame_period(60.0);
        assert!(p60 > Duration::from_millis(15) && p60 < Duration::from_millis(18));
    }
}
