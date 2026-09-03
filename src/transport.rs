use std::time::Duration;

use egui::{Align2, Color32, CornerRadius, Sense, Ui, Vec2};

use crate::icons::{Icon, IconCache};
use crate::theme;

const STEP_SMALL: usize = 1;

const STEP_BUTTON_W: f32 = 34.0;
const STEP_BUTTON_H: f32 = 30.0;
const PLAY_BUTTON_W: f32 = 48.0;
const PLAY_BUTTON_H: f32 = 30.0;
const STEP_ICON_PT: f32 = 22.0;
const PLAY_ICON_PT: f32 = 28.0;
const BUTTON_GAP: f32 = 4.0;
const STEP_BUTTON_COUNT: f32 = 4.0;
const ROW_WIDTH: f32 = STEP_BUTTON_W * STEP_BUTTON_COUNT + PLAY_BUTTON_W + BUTTON_GAP * 4.0;

#[derive(Debug, Clone, PartialEq)]
pub enum TransportAction {
    Home,
    Back(usize),
    Fwd(usize),
    End,
    TogglePlay,
    ToggleTrim,
    ResetTrim,
    ScaleChanged(f32),
    FrameInputChanged(String),
    FrameInputCommit(String),
    FrameInputCancel,
}

#[derive(Debug, Clone)]
pub struct TransportView {
    pub enabled: bool,
    pub playing: bool,
    pub trim_mode: bool,
    pub can_reset_trim: bool,
    pub strip_scale: f32,
    pub fps: f32,
    pub frame_input_edit: Option<String>,
    pub frame_input_needs_focus: bool,
}

/// Draw the centered transport button row. Returns the first pressed action
/// and the updated needs_focus flag.
pub fn draw(
    ui: &mut Ui,
    icons: &mut IconCache,
    mut view: TransportView,
    current_frame: usize,
    total_frames: usize,
) -> (Option<TransportAction>, bool) {
    let mut action: Option<TransportAction> = None;
    let mut new_scale = view.strip_scale;

    ui.vertical(|ui| {
        let badge_action = ui
            .with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                render_frame_badge(
                    ui,
                    current_frame + 1,
                    total_frames,
                    view.frame_input_edit.as_ref(),
                    &mut view.frame_input_needs_focus,
                )
            })
            .inner;
        if action.is_none() && badge_action.is_some() {
            action = badge_action;
        }

        ui.horizontal(|ui| {
            let row_start = ui.cursor().left();
            let total_w = ui.available_width();
            let row_center = row_start + total_w * 0.5;

            ui.spacing_mut().item_spacing.x = BUTTON_GAP;

            let trim_color = if view.trim_mode {
                theme::ACCENT
            } else {
                Color32::WHITE
            };
            if icons
                .ui(
                    ui,
                    Icon::Scissors,
                    20.0,
                    trim_color,
                    Vec2::new(STEP_BUTTON_W, STEP_BUTTON_H),
                    false,
                )
                .on_hover_text("Enable trim mode — gate the active frame range with yellow handles")
                .clicked()
            {
                action = Some(TransportAction::ToggleTrim);
            }
            if view.trim_mode && view.can_reset_trim {
                if icons
                    .ui(
                        ui,
                        Icon::RotateCcw,
                        18.0,
                        theme::TEXT_MUTED,
                        Vec2::new(STEP_BUTTON_W, STEP_BUTTON_H),
                        false,
                    )
                    .on_hover_text("Reset trim to full clip")
                    .clicked()
                {
                    action = Some(TransportAction::ResetTrim);
                }
            }
            ui.add_space(BUTTON_GAP);
            let after_trim_x = ui.cursor().left();

            let target_playback_left = row_center - ROW_WIDTH * 0.5;
            let pad_before = (target_playback_left - after_trim_x).max(0.0);
            ui.add_space(pad_before);

            if step_button(
                ui,
                icons,
                Icon::SkipBack,
                view.enabled,
                "First frame (Home)",
            ) {
                action = Some(TransportAction::Home);
            }
            if step_button(
                ui,
                icons,
                Icon::StepBack,
                view.enabled,
                "Back 1 frame (Left / ,) — Shift for 1 second",
            ) {
                let shift_held = ui.input(|i| i.modifiers.shift);
                let step = if shift_held {
                    view.fps.round().max(1.0) as usize
                } else {
                    STEP_SMALL
                };
                action = Some(TransportAction::Back(step));
            }
            if play_button(ui, icons, &view) {
                action = Some(TransportAction::TogglePlay);
            }
            if step_button(
                ui,
                icons,
                Icon::StepForward,
                view.enabled,
                "Forward 1 frame (Right / .) — Shift for 1 second",
            ) {
                let shift_held = ui.input(|i| i.modifiers.shift);
                let step = if shift_held {
                    view.fps.round().max(1.0) as usize
                } else {
                    STEP_SMALL
                };
                action = Some(TransportAction::Fwd(step));
            }
            if step_button(
                ui,
                icons,
                Icon::SkipForward,
                view.enabled,
                "Last frame (End)",
            ) {
                action = Some(TransportAction::End);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let slider = egui::Slider::new(&mut new_scale, 0.0..=1.0)
                    .show_value(false)
                    .min_decimals(0)
                    .max_decimals(2);
                let slider_resp = ui.add_enabled(view.enabled, slider.fixed_decimals(2));
                if slider_resp.changed() {
                    action = Some(TransportAction::ScaleChanged(new_scale));
                }
            });
        });
    });

    (action, view.frame_input_needs_focus)
}

fn render_frame_badge(
    ui: &mut Ui,
    current: usize,
    total: usize,
    editing: Option<&String>,
    needs_focus: &mut bool,
) -> Option<TransportAction> {
    use egui::TextEdit;

    let display_text = format!("{} / {}", current, total.max(1));
    let galley = ui.painter().layout_no_wrap(
        display_text.clone(),
        egui::FontId::monospace(12.0),
        theme::TEXT_MUTED,
    );
    let chip_size = galley.size() + Vec2::new(12.0, 6.0);

    ui.allocate_ui_with_layout(
        chip_size,
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            if let Some(buffer) = editing {
                let mut text = buffer.clone();
                let response = ui.add(
                    TextEdit::singleline(&mut text)
                        .desired_width(chip_size.x)
                        .min_size(chip_size)
                        .font(egui::FontId::monospace(12.0)),
                );

                if *needs_focus {
                    response.request_focus();
                    if response.has_focus() {
                        *needs_focus = false;
                    }
                }

                if response.lost_focus() {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        return Some(TransportAction::FrameInputCancel);
                    }
                    return Some(TransportAction::FrameInputCommit(text));
                }
                if text != *buffer {
                    return Some(TransportAction::FrameInputChanged(text));
                }
                None
            } else {
                let (rect, response) = ui.allocate_exact_size(chip_size, Sense::click());
                let bg = if response.hovered() {
                    theme::WIDGET_HOVERED
                } else {
                    theme::WIDGET_IDLE
                };
                ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    &display_text,
                    egui::FontId::monospace(12.0),
                    theme::TEXT_MUTED,
                );
                if response.clicked() {
                    return Some(TransportAction::FrameInputChanged(current.to_string()));
                }
                None
            }
        },
    )
    .inner
}

fn step_button(
    ui: &mut Ui,
    icons: &mut IconCache,
    icon: Icon,
    enabled: bool,
    tooltip: &str,
) -> bool {
    let color = if enabled {
        Color32::WHITE
    } else {
        theme::TEXT_MUTED
    };
    ui.add_enabled_ui(enabled, |ui| {
        icons
            .ui(
                ui,
                icon,
                STEP_ICON_PT,
                color,
                Vec2::new(STEP_BUTTON_W, STEP_BUTTON_H),
                false,
            )
            .on_hover_text(tooltip)
            .clicked()
    })
    .inner
}

fn play_button(ui: &mut Ui, icons: &mut IconCache, view: &TransportView) -> bool {
    let icon = if view.playing {
        Icon::Pause
    } else {
        Icon::Play
    };
    let color = if view.enabled {
        Color32::WHITE
    } else {
        theme::TEXT_MUTED
    };
    let tooltip = if view.playing {
        "Pause (Space)"
    } else {
        "Play (Space)"
    };
    ui.add_enabled_ui(view.enabled, |ui| {
        icons
            .ui(
                ui,
                icon,
                PLAY_ICON_PT,
                color,
                Vec2::new(PLAY_BUTTON_W, PLAY_BUTTON_H),
                false,
            )
            .on_hover_text(tooltip)
            .clicked()
    })
    .inner
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

    #[test]
    fn fps_step_rounds_29_97_to_30() {
        let fps = 29.97_f32;
        let step = fps.round().max(1.0) as usize;
        assert_eq!(step, 30);
    }
}
