use egui::{
    scroll_area::{DragScroll, ScrollBarVisibility, ScrollSource},
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, ScrollArea, Sense, Stroke, Ui,
    Vec2,
};

use crate::theme;
use crate::thumbs::ThumbCache;

pub const TRIM_HANDLE_W: f32 = 22.0;
pub const TRIM_RAIL_H: f32 = 4.0;

/// Distance from a viewport edge, in pixels, that triggers edge auto-scroll
/// during a trim handle drag.
pub const EDGE_DEAD_ZONE_PX: f32 = 40.0;

/// Peak scroll speed when the pointer sits at the extreme edge of the
/// filmstrip viewport during a trim handle drag.
pub const MAX_SCROLL_SPEED_PX_PER_S: f32 = 400.0;

/// Compute the scroll velocity (px/s, positive = scroll right) to apply to
/// the filmstrip while a trim handle is being dragged near a viewport edge.
/// Returns 0.0 when the pointer sits in the central "safe" zone.
pub fn edge_scroll_velocity(pointer_x: f32, viewport_rect: Rect) -> f32 {
    let left_edge = viewport_rect.min.x;
    let right_edge = viewport_rect.max.x;
    if pointer_x < left_edge + EDGE_DEAD_ZONE_PX {
        let depth = (left_edge + EDGE_DEAD_ZONE_PX - pointer_x).clamp(0.0, EDGE_DEAD_ZONE_PX);
        -MAX_SCROLL_SPEED_PX_PER_S * (depth / EDGE_DEAD_ZONE_PX)
    } else if pointer_x > right_edge - EDGE_DEAD_ZONE_PX {
        let depth = (pointer_x - (right_edge - EDGE_DEAD_ZONE_PX)).clamp(0.0, EDGE_DEAD_ZONE_PX);
        MAX_SCROLL_SPEED_PX_PER_S * (depth / EDGE_DEAD_ZONE_PX)
    } else {
        0.0
    }
}

/// Compute the minimal stride that fits the entire clip in the viewport.
/// Returns the smallest stride where `ceil(total_frames / stride)` thumbs fit
/// within `floor(viewport_width / pitch)` positions.
pub fn fit_clip_stride(total_frames: usize, viewport_width: f32, pitch: f32) -> usize {
    if total_frames == 0 || pitch <= 0.0 {
        return 1;
    }
    let positions = (viewport_width / pitch).floor().max(1.0) as usize;
    ((total_frames as f32) / (positions as f32)).ceil().max(1.0) as usize
}

/// Given a slider position in [0.0, 1.0], interpolate (log scale) between
/// `fit_clip_stride` (slider = 0.0) and 1 (slider = 1.0).
pub fn stride_from_scale(
    scale: f32,
    total_frames: usize,
    viewport_width: f32,
    pitch: f32,
) -> usize {
    let max_stride = fit_clip_stride(total_frames, viewport_width, pitch);
    if max_stride <= 1 {
        return 1;
    }
    let scale_clamped = scale.clamp(0.0, 1.0);
    let log_min = 0.0_f32;
    let log_max = (max_stride as f32).ln();
    let log_val = log_max - scale_clamped * (log_max - log_min);
    log_val.exp().round().max(1.0) as usize
}

/// Convert a frame index to a strip position given the current stride.
pub fn frame_to_strip_pos(frame: usize, stride: usize) -> usize {
    frame / stride.max(1)
}

/// Convert a strip position to a frame index given the current stride.
pub fn strip_pos_to_frame(strip_pos: usize, stride: usize) -> usize {
    strip_pos * stride.max(1)
}

/// Which of the two trim handles a drag is affecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimHandle {
    Start,
    End,
}

#[derive(Debug, Clone, Copy)]
pub struct FilmstripGeometry {
    pub thumb_w: f32,
    pub thumb_h: f32,
    pub gap: f32,
    pub top_pad: f32,
    pub label_h: f32,
    pub label_every_n: usize,
    /// Extra thumbnails to prefetch on each side of the visible range.
    pub prefetch_pad: usize,
    /// Left/right padding for center-active filmstrip (0.0 for tests).
    pub left_pad: f32,
}

impl Default for FilmstripGeometry {
    fn default() -> Self {
        Self {
            thumb_w: 96.0,
            thumb_h: 96.0,
            gap: 4.0,
            top_pad: 4.0,
            label_h: 14.0,
            label_every_n: 5,
            prefetch_pad: 30,
            left_pad: 0.0,
        }
    }
}

impl FilmstripGeometry {
    pub fn pitch(&self) -> f32 {
        self.thumb_w + self.gap
    }

    pub fn total_width(&self, count: usize) -> f32 {
        if count == 0 {
            0.0
        } else {
            count as f32 * self.pitch() - self.gap
        }
    }

    pub fn row_height(&self) -> f32 {
        self.top_pad + self.thumb_h + self.label_h
    }

    /// Compute the inclusive index range of thumbnails that intersect a
    /// horizontal viewport `[x0, x1]` given a strip of `count` thumbnails.
    /// Returns `None` when there are no thumbnails.
    pub fn visible_range(&self, x0: f32, x1: f32, count: usize) -> Option<(usize, usize)> {
        if count == 0 {
            return None;
        }
        let last = count - 1;
        let pitch = self.pitch();
        if pitch <= 0.0 {
            return Some((0, last));
        }
        let x0_adj = (x0 - self.left_pad).max(0.0);
        let x1_adj = (x1 - self.left_pad).max(0.0);
        let first = (x0_adj / pitch).floor().max(0.0) as usize;
        let last_seen = ((x1_adj - self.gap) / pitch).ceil() as isize;
        let last_seen = last_seen.max(0) as usize;
        let first = first.min(last);
        let last_visible = last_seen.min(last);
        Some((first, last_visible.max(first)))
    }

    /// Range of thumbnails to keep decoded (visible + prefetch pad on each side).
    pub fn prefetch_range(
        &self,
        visible: Option<(usize, usize)>,
        count: usize,
    ) -> Option<(usize, usize)> {
        let (lo, hi) = visible?;
        if count == 0 {
            return None;
        }
        let last = count - 1;
        let pad = self.prefetch_pad;
        let lo = lo.saturating_sub(pad);
        let hi = (hi + pad).min(last);
        Some((lo, hi))
    }

    pub fn thumb_rect(&self, container_origin: Pos2, index: usize) -> Rect {
        let x = container_origin.x + self.left_pad + index as f32 * self.pitch();
        let y = container_origin.y + self.top_pad;
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(self.thumb_w, self.thumb_h))
    }

    /// Rect for a trim handle, centered on the boundary of the given thumb
    /// (left edge for `Start`, right edge for `End`). The handle spans the
    /// filmstrip height.
    pub fn trim_handle_rect(&self, container_origin: Pos2, index: usize, side: TrimHandle) -> Rect {
        let thumb = self.thumb_rect(container_origin, index);
        let center_x = match side {
            TrimHandle::Start => thumb.left(),
            TrimHandle::End => thumb.right(),
        };
        let top = container_origin.y + self.top_pad - 2.0;
        let bottom = container_origin.y + self.top_pad + self.thumb_h + 2.0;
        Rect::from_min_max(
            Pos2::new(center_x - TRIM_HANDLE_W * 0.5, top),
            Pos2::new(center_x + TRIM_HANDLE_W * 0.5, bottom),
        )
    }
}

/// Output of one draw pass of the filmstrip.
#[derive(Debug, Default, Clone, Copy)]
pub struct FilmstripAction {
    /// The user clicked or dragged onto this frame; caller should seek to it.
    pub seek_to: Option<usize>,
    /// The user changed `current_frame` before this frame; scroll it into view.
    pub scroll_into_view: bool,
    /// A trim handle drag started this frame — caller snapshots the current
    /// `(trim_start, trim_end)` so drag-release can record one history entry.
    pub trim_drag_started: Option<TrimHandle>,
    /// A trim handle drag ended this frame — caller records the change against
    /// the snapshot it took on drag-start.
    pub trim_drag_stopped: bool,
}

pub struct FilmstripDrawParams<'a> {
    pub geom: FilmstripGeometry,
    pub total_frames: usize,
    pub current_frame: usize,
    pub prev_current_frame: usize,
    pub trim_mode: bool,
    /// Mutable so the strip can update on handle drag.
    pub trim_start: &'a mut usize,
    pub trim_end: &'a mut usize,
    pub thumbs: &'a mut ThumbCache,
    /// Accumulator for scroll-to-scrub, converted to frame steps.
    pub scroll_accumulator: &'a mut f32,
    /// When `Some`, overrides the default current-frame-centered scroll offset
    /// with this raw content-space value. Used to hold the auto-scrolled strip
    /// position while a trim handle is being edge-dragged; the accumulator IS
    /// the sub-pixel state (offset is a plain `f32`). Cleared to `None` when
    /// the trim drag ends so the strip snaps back to centering `current_frame`.
    pub trim_scroll_override: &'a mut Option<f32>,
    pub stride: usize,
}

/// Draw the filmstrip inside `ui`, returning a request for the caller to seek
/// somewhere. The strip fills the width and its height is `geom.row_height()`.
pub fn draw(ui: &mut Ui, params: FilmstripDrawParams<'_>) -> FilmstripAction {
    let FilmstripDrawParams {
        mut geom,
        total_frames,
        current_frame,
        prev_current_frame,
        trim_mode,
        trim_start,
        trim_end,
        thumbs,
        scroll_accumulator,
        trim_scroll_override,
        stride,
    } = params;

    let mut action = FilmstripAction::default();
    if total_frames == 0 {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("no frames yet")
                    .color(theme::TEXT_MUTED)
                    .small(),
            );
        });
        return action;
    }

    // Whether the yellow handles are meaningful. With fewer than two frames
    // there's nothing to trim; render just the plain strip in that case.
    let trim_enabled = trim_mode && total_frames >= 2;

    let stride = stride.max(1);
    let strip_positions = ((total_frames as f32) / (stride as f32)).ceil() as usize;
    let content_width = geom.total_width(strip_positions);
    let row_h = geom.row_height();
    let want_scroll = current_frame != prev_current_frame;

    let strip_pos = frame_to_strip_pos(current_frame, stride);
    let default_target_offset = strip_pos as f32 * geom.pitch() + geom.thumb_w * 0.5;
    let scroll_offset = trim_scroll_override.unwrap_or(default_target_offset);

    ScrollArea::horizontal()
        .id_salt("frammpeg-filmstrip")
        .auto_shrink([false, false])
        .horizontal_scroll_offset(scroll_offset)
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
        .scroll_source(ScrollSource {
            scroll_bar: false,
            drag: DragScroll::Never,
            mouse_wheel: false,
        })
        .show_viewport(ui, |ui, viewport| {
            let left_pad = viewport.width() * 0.5;
            let right_pad = left_pad;
            geom.left_pad = left_pad;
            let padded_width = content_width + left_pad + right_pad;
            let (content_rect, seek_response) = ui.allocate_exact_size(
                Vec2::new(padded_width.max(viewport.width()), row_h),
                Sense::click_and_drag(),
            );
            let painter = ui.painter_at(content_rect);
            painter.rect_filled(content_rect, CornerRadius::ZERO, theme::PANEL);

            let x0 = viewport.min.x;
            let x1 = viewport.max.x;
            let visible_strip = geom.visible_range(x0, x1, strip_positions);

            let visible_frames = visible_strip.map(|(lo, hi)| {
                let lo_frame = strip_pos_to_frame(lo, stride);
                let hi_frame = strip_pos_to_frame(hi, stride).min(total_frames.saturating_sub(1));
                (lo_frame, hi_frame)
            });

            let prefetch = geom.prefetch_range(visible_frames, total_frames);

            if let Some((lo, hi)) = prefetch {
                let mut frame = lo;
                while frame <= hi {
                    thumbs.request(frame);
                    frame = frame.saturating_add(stride).min(hi.saturating_add(1));
                }
            }

            if let Some((lo, hi)) = visible_strip {
                paint_thumbs(
                    &painter,
                    content_rect.min,
                    geom,
                    thumbs,
                    (lo, hi),
                    current_frame,
                    (*trim_start, *trim_end),
                    trim_enabled,
                    stride,
                );
            }

            if trim_enabled {
                let trim_start_pos = frame_to_strip_pos(*trim_start, stride);
                let trim_end_pos = frame_to_strip_pos(*trim_end, stride);
                paint_trim_rails(
                    &painter,
                    content_rect.min,
                    geom,
                    (trim_start_pos, trim_end_pos),
                );
            }

            if want_scroll {
                action.scroll_into_view = true;
            }

            let screen_viewport = Rect::from_min_max(
                content_rect.min + viewport.min.to_vec2(),
                content_rect.min + viewport.max.to_vec2(),
            );

            let mut handle_pointer_captured = false;
            if trim_enabled {
                handle_pointer_captured = handle_trim_interaction(
                    ui,
                    content_rect,
                    screen_viewport,
                    geom,
                    total_frames,
                    trim_start,
                    trim_end,
                    stride,
                    content_width,
                    trim_scroll_override,
                    &mut action,
                );
            } else if trim_scroll_override.is_some() {
                *trim_scroll_override = None;
            }

            if !handle_pointer_captured {
                handle_seek_interaction(
                    &seek_response,
                    content_rect,
                    geom,
                    total_frames,
                    (*trim_start, *trim_end),
                    trim_enabled,
                    stride,
                    &mut action,
                );
            }

            if seek_response.hovered() {
                let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
                let delta_combined = scroll_delta.x + scroll_delta.y;
                *scroll_accumulator += delta_combined;

                let pitch = geom.pitch();
                if pitch > 0.0 && scroll_accumulator.abs() >= pitch {
                    let frames_to_seek = (scroll_accumulator.abs() / pitch).floor() as isize;
                    if frames_to_seek > 0 {
                        let direction = if *scroll_accumulator > 0.0 { 1 } else { -1 };
                        let new_frame = (current_frame as isize + direction * frames_to_seek)
                            .max(0)
                            .min(total_frames.saturating_sub(1) as isize)
                            as usize;
                        let clamped_frame = if trim_enabled {
                            new_frame.clamp(*trim_start, *trim_end)
                        } else {
                            new_frame
                        };
                        action.seek_to = Some(clamped_frame);
                        *scroll_accumulator -= direction as f32 * frames_to_seek as f32 * pitch;
                    }
                }
            }
        });

    action
}

#[allow(clippy::too_many_arguments)]
fn paint_thumbs(
    painter: &egui::Painter,
    origin: Pos2,
    geom: FilmstripGeometry,
    thumbs: &mut ThumbCache,
    range: (usize, usize),
    current_frame: usize,
    trim: (usize, usize),
    trim_enabled: bool,
    stride: usize,
) {
    let (lo, hi) = range;
    let (trim_start, trim_end) = trim;
    let stride = stride.max(1);
    let current_strip_pos = frame_to_strip_pos(current_frame, stride);
    for strip_pos in lo..=hi {
        let frame_idx = strip_pos_to_frame(strip_pos, stride);
        let rect = geom.thumb_rect(origin, strip_pos);
        let selected = strip_pos == current_strip_pos;
        let in_trim = !trim_enabled || (frame_idx >= trim_start && frame_idx <= trim_end);
        let bg = if selected {
            theme::WIDGET_ACTIVE
        } else {
            theme::WIDGET_IDLE
        };
        painter.rect_filled(rect, CornerRadius::same(2), bg);

        if let Some(tex) = thumbs.get(frame_idx) {
            let [tw, th] = tex.size();
            let tw = tw as f32;
            let th = th as f32;
            if tw > 0.0 && th > 0.0 {
                let scale = (rect.width() / tw).min(rect.height() / th);
                let disp = Vec2::new(tw * scale, th * scale);
                let inner = Rect::from_center_size(rect.center(), disp);
                painter.image(
                    tex.id(),
                    inner,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        } else {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "…",
                FontId::proportional(12.0),
                theme::TEXT_MUTED,
            );
        }

        // Dim thumbs outside the trim range with a semi-transparent black
        // overlay so the excluded frames are still legible.
        if !in_trim {
            painter.rect_filled(
                rect,
                CornerRadius::same(2),
                Color32::from_rgba_unmultiplied(0x08, 0x0A, 0x0D, 155),
            );
        }

        let (stroke_w, stroke_c) = if selected {
            (2.0, theme::ACCENT)
        } else {
            (1.0, theme::STROKE)
        };
        painter.rect_stroke(
            rect,
            CornerRadius::same(2),
            Stroke::new(stroke_w, stroke_c),
            egui::StrokeKind::Outside,
        );

        if (strip_pos + 1) % geom.label_every_n.max(1) == 0 || selected {
            let label_pos = Pos2::new(rect.center().x, rect.bottom() + geom.label_h * 0.5);
            let color = if selected {
                theme::ACCENT
            } else if !in_trim {
                Color32::from_rgba_unmultiplied(0x8A, 0x92, 0x9B, 170)
            } else {
                theme::TEXT_MUTED
            };
            painter.text(
                label_pos,
                Align2::CENTER_CENTER,
                format!("{}", frame_idx + 1),
                FontId::monospace(10.0),
                color,
            );
        }
    }
}

fn paint_trim_rails(
    painter: &egui::Painter,
    origin: Pos2,
    geom: FilmstripGeometry,
    trim: (usize, usize),
) {
    let (trim_start, trim_end) = trim;
    let start_thumb = geom.thumb_rect(origin, trim_start);
    let end_thumb = geom.thumb_rect(origin, trim_end);
    let rail_left = start_thumb.left();
    let rail_right = end_thumb.right();
    let top = origin.y + geom.top_pad - TRIM_RAIL_H * 0.5;
    let bottom = origin.y + geom.top_pad + geom.thumb_h - TRIM_RAIL_H * 0.5;
    let rail_top = Rect::from_min_max(
        Pos2::new(rail_left, top),
        Pos2::new(rail_right, top + TRIM_RAIL_H),
    );
    let rail_bottom = Rect::from_min_max(
        Pos2::new(rail_left, bottom),
        Pos2::new(rail_right, bottom + TRIM_RAIL_H),
    );
    painter.rect_filled(rail_top, CornerRadius::same(1), theme::TRIM_ACCENT);
    painter.rect_filled(rail_bottom, CornerRadius::same(1), theme::TRIM_ACCENT);
}

fn paint_trim_handle(painter: &egui::Painter, rect: Rect, hovered_or_dragged: bool) {
    let fill = if hovered_or_dragged {
        theme::TRIM_ACCENT_HOVERED
    } else {
        theme::TRIM_ACCENT
    };
    painter.rect_filled(rect, CornerRadius::same(4), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x00, 0x00, 0x00, 120)),
        egui::StrokeKind::Outside,
    );
    // Two vertical grip lines for a more tactile grabber feel.
    let cx = rect.center().x;
    let grip_top = rect.top() + rect.height() * 0.35;
    let grip_bottom = rect.bottom() - rect.height() * 0.35;
    let grip_stroke = Stroke::new(2.0, Color32::from_rgba_unmultiplied(0x00, 0x00, 0x00, 160));
    painter.line_segment(
        [
            Pos2::new(cx - 3.0, grip_top),
            Pos2::new(cx - 3.0, grip_bottom),
        ],
        grip_stroke,
    );
    painter.line_segment(
        [
            Pos2::new(cx + 3.0, grip_top),
            Pos2::new(cx + 3.0, grip_bottom),
        ],
        grip_stroke,
    );
}

/// Returns true if either handle currently owns the pointer (drag started or
/// active), so the caller can skip the strip's own seek handling.
#[allow(clippy::too_many_arguments)]
fn handle_trim_interaction(
    ui: &mut Ui,
    content_rect: Rect,
    screen_viewport: Rect,
    geom: FilmstripGeometry,
    total_frames: usize,
    trim_start: &mut usize,
    trim_end: &mut usize,
    stride: usize,
    content_width: f32,
    trim_scroll_override: &mut Option<f32>,
    action: &mut FilmstripAction,
) -> bool {
    let last = total_frames.saturating_sub(1);
    let stride = stride.max(1);
    let start_strip_pos = frame_to_strip_pos(*trim_start, stride);
    let end_strip_pos = frame_to_strip_pos(*trim_end, stride);
    let start_rect = geom.trim_handle_rect(content_rect.min, start_strip_pos, TrimHandle::Start);
    let end_rect = geom.trim_handle_rect(content_rect.min, end_strip_pos, TrimHandle::End);

    let start_id = ui.id().with("frammpeg-trim-start");
    let end_id = ui.id().with("frammpeg-trim-end");
    let start_resp = ui.interact(start_rect, start_id, Sense::click_and_drag());
    let end_resp = ui.interact(end_rect, end_id, Sense::click_and_drag());

    let painter = ui.painter_at(content_rect);
    paint_trim_handle(
        &painter,
        start_rect,
        start_resp.hovered() || start_resp.dragged(),
    );
    paint_trim_handle(&painter, end_rect, end_resp.hovered() || end_resp.dragged());

    if start_resp.hovered() || end_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }

    if start_resp.drag_started() {
        action.trim_drag_started = Some(TrimHandle::Start);
    } else if end_resp.drag_started() {
        action.trim_drag_started = Some(TrimHandle::End);
    }

    if start_resp.dragged() {
        if let Some(pos) = start_resp.interact_pointer_pos() {
            let x = pos.x - content_rect.min.x - geom.left_pad;
            let pitch = geom.pitch();
            if pitch > 0.0 {
                let strip_idx = (x / pitch).floor().max(0.0) as usize;
                let frame = strip_pos_to_frame(strip_idx, stride).min(last);
                let clamped_hi = trim_end.saturating_sub(1);
                *trim_start = frame.min(clamped_hi);
            }
        }
    }
    if end_resp.dragged() {
        if let Some(pos) = end_resp.interact_pointer_pos() {
            let x = pos.x - content_rect.min.x - geom.left_pad;
            let pitch = geom.pitch();
            if pitch > 0.0 {
                let strip_idx = (x / pitch).floor().max(0.0) as usize;
                let frame = strip_pos_to_frame(strip_idx, stride).min(last);
                let clamped_lo = trim_start.saturating_add(1);
                *trim_end = frame.max(clamped_lo).min(last);
            }
        }
    }

    let is_dragging = start_resp.dragged() || end_resp.dragged();
    if is_dragging {
        let pointer_pos = start_resp
            .interact_pointer_pos()
            .or_else(|| end_resp.interact_pointer_pos());
        if let Some(pos) = pointer_pos {
            let velocity = edge_scroll_velocity(pos.x, screen_viewport);
            if velocity != 0.0 {
                let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.1);
                let current = trim_scroll_override
                    .unwrap_or((screen_viewport.min.x - content_rect.min.x).max(0.0));
                let new_offset = (current + velocity * dt).clamp(0.0, content_width.max(0.0));
                *trim_scroll_override = Some(new_offset);
                ui.ctx().request_repaint();
            }
        }
    } else if trim_scroll_override.is_some() {
        *trim_scroll_override = None;
    }

    if start_resp.drag_stopped() || end_resp.drag_stopped() {
        action.trim_drag_stopped = true;
    }

    start_resp.hovered()
        || end_resp.hovered()
        || start_resp.dragged()
        || end_resp.dragged()
        || start_resp.drag_stopped()
        || end_resp.drag_stopped()
        || start_resp.clicked()
        || end_resp.clicked()
}

#[allow(clippy::too_many_arguments)]
fn handle_seek_interaction(
    response: &Response,
    content_rect: Rect,
    geom: FilmstripGeometry,
    total_frames: usize,
    trim: (usize, usize),
    trim_enabled: bool,
    stride: usize,
    action: &mut FilmstripAction,
) {
    if total_frames == 0 {
        return;
    }
    let pitch = geom.pitch();
    if pitch <= 0.0 {
        return;
    }
    let stride = stride.max(1);
    let last = total_frames.saturating_sub(1);
    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let x = (pos.x - content_rect.min.x - geom.left_pad) / pitch;
            let strip_idx = (x.floor() as isize).max(0) as usize;
            let frame = strip_pos_to_frame(strip_idx, stride).min(last);
            let target = if trim_enabled {
                frame.clamp(trim.0, trim.1)
            } else {
                frame
            };
            action.seek_to = Some(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom() -> FilmstripGeometry {
        FilmstripGeometry::default()
    }

    #[test]
    fn visible_range_covers_viewport() {
        let g = geom(); // pitch = 100
        let (lo, hi) = g.visible_range(0.0, 200.0, 100).unwrap();
        assert_eq!(lo, 0);
        // 200 / 100 = 2.0 -> ceil = 2, so at least index 1 is visible.
        assert!(hi >= 1);
    }

    #[test]
    fn visible_range_clamps_past_end() {
        let g = geom();
        let (lo, hi) = g.visible_range(0.0, 10_000.0, 5).unwrap();
        assert_eq!(lo, 0);
        assert_eq!(hi, 4);
    }

    #[test]
    fn visible_range_starts_mid_strip() {
        let g = geom(); // pitch = 100
        let (lo, hi) = g.visible_range(200.0, 400.0, 100).unwrap();
        // 200 / 100 = 2.0 -> floor 2
        assert_eq!(lo, 2);
        assert!(hi >= 3);
    }

    #[test]
    fn visible_range_returns_none_for_zero_frames() {
        let g = geom();
        assert!(g.visible_range(0.0, 100.0, 0).is_none());
    }

    #[test]
    fn prefetch_range_pads_symmetrically() {
        let g = FilmstripGeometry {
            prefetch_pad: 3,
            ..geom()
        };
        let range = g.prefetch_range(Some((10, 15)), 100).unwrap();
        assert_eq!(range, (7, 18));
    }

    #[test]
    fn prefetch_range_clamps_ends() {
        let g = FilmstripGeometry {
            prefetch_pad: 5,
            ..geom()
        };
        let range = g.prefetch_range(Some((0, 4)), 8).unwrap();
        assert_eq!(range, (0, 7));
    }

    #[test]
    fn total_width_matches_pitch_math() {
        let g = geom();
        let w = g.total_width(10);
        // 10 * 100 - 4 = 996
        assert!((w - 996.0).abs() < 0.001);
    }

    #[test]
    fn total_width_zero_frames() {
        let g = geom();
        assert_eq!(g.total_width(0), 0.0);
    }

    #[test]
    fn trim_handle_rect_centered_on_thumb_boundary() {
        let g = geom(); // pitch 100, thumb_w 96
        let origin = Pos2::new(0.0, 0.0);
        let start = g.trim_handle_rect(origin, 3, TrimHandle::Start);
        // Start handle is centered on thumb.left() = 3 * 100 = 300.
        assert!((start.center().x - 300.0).abs() < 0.001);
        assert!((start.width() - TRIM_HANDLE_W).abs() < 0.001);

        let end = g.trim_handle_rect(origin, 3, TrimHandle::End);
        // End handle is centered on thumb.right() = 3 * 100 + 96 = 396.
        assert!((end.center().x - 396.0).abs() < 0.001);
    }

    #[test]
    fn trim_handle_rect_spans_full_strip_height() {
        let g = geom();
        let origin = Pos2::new(10.0, 20.0);
        let rect = g.trim_handle_rect(origin, 0, TrimHandle::Start);
        // Spans slightly past the thumb top/bottom for a clean grabber shape.
        assert!(rect.top() <= origin.y + g.top_pad);
        assert!(rect.bottom() >= origin.y + g.top_pad + g.thumb_h);
    }

    #[test]
    fn trim_handle_hit_test_prioritizes_over_thumb() {
        // At the boundary between thumb index N and N+1, the trim_start handle
        // sitting at thumb N.left() overlaps the tail end of thumb N-1 and the
        // head of thumb N. Interactive registration puts handles later in the
        // draw so egui's interact resolution favours them — this is a
        // geometry-level sanity check that the handle rect actually contains
        // the boundary point.
        let g = geom();
        let origin = Pos2::new(0.0, 0.0);
        let handle = g.trim_handle_rect(origin, 5, TrimHandle::Start);
        let boundary = Pos2::new(5.0 * g.pitch(), origin.y + g.top_pad + g.thumb_h * 0.5);
        assert!(handle.contains(boundary));
    }

    #[test]
    fn fit_clip_stride_math() {
        let pitch = 100.0;
        let viewport_width = 1000.0;
        let total_frames = 500;
        let stride = fit_clip_stride(total_frames, viewport_width, pitch);
        let positions = (viewport_width / pitch).floor() as usize;
        let thumbs_needed = ((total_frames as f32) / (stride as f32)).ceil() as usize;
        assert!(thumbs_needed <= positions);
        if stride > 1 {
            let smaller_stride = stride - 1;
            let thumbs_with_smaller =
                ((total_frames as f32) / (smaller_stride as f32)).ceil() as usize;
            assert!(thumbs_with_smaller > positions);
        }
    }

    #[test]
    fn stride_from_slider_position_endpoints() {
        let total_frames = 500;
        let viewport_width = 1000.0;
        let pitch = 100.0;
        let max_stride = fit_clip_stride(total_frames, viewport_width, pitch);
        assert_eq!(
            stride_from_scale(0.0, total_frames, viewport_width, pitch),
            max_stride
        );
        assert_eq!(
            stride_from_scale(1.0, total_frames, viewport_width, pitch),
            1
        );
    }

    #[test]
    fn stride_from_slider_position_midpoint() {
        let total_frames = 500;
        let viewport_width = 1000.0;
        let pitch = 100.0;
        let max_stride = fit_clip_stride(total_frames, viewport_width, pitch);
        let mid_stride = stride_from_scale(0.5, total_frames, viewport_width, pitch);
        assert!(mid_stride > 1);
        assert!(mid_stride < max_stride);
    }

    #[test]
    fn strip_scale_default_produces_stride_1() {
        let default_scale = 1.0;
        assert_eq!(stride_from_scale(default_scale, 100, 800.0, 100.0), 1);
        assert_eq!(stride_from_scale(default_scale, 500, 1000.0, 100.0), 1);
        assert_eq!(stride_from_scale(default_scale, 1000, 1920.0, 100.0), 1);
    }

    #[test]
    fn strip_position_of_frame() {
        assert_eq!(frame_to_strip_pos(120, 10), 12);
        assert_eq!(frame_to_strip_pos(0, 10), 0);
        assert_eq!(frame_to_strip_pos(125, 10), 12);
    }

    #[test]
    fn frame_of_strip_position() {
        assert_eq!(strip_pos_to_frame(12, 10), 120);
        assert_eq!(strip_pos_to_frame(0, 10), 0);
        assert_eq!(strip_pos_to_frame(5, 1), 5);
    }

    #[test]
    fn edge_scroll_velocity_maps_pointer_to_direction_and_speed() {
        let viewport = Rect::from_min_max(Pos2::new(100.0, 0.0), Pos2::new(500.0, 40.0));

        // Pointer past the left edge: fastest negative scroll (leftward).
        let v_left = edge_scroll_velocity(viewport.min.x - 10.0, viewport);
        assert!((v_left - -MAX_SCROLL_SPEED_PX_PER_S).abs() < 0.001);

        // Pointer at the center: no scrolling.
        let center = (viewport.min.x + viewport.max.x) * 0.5;
        assert_eq!(edge_scroll_velocity(center, viewport), 0.0);

        // Pointer past the right edge: fastest positive scroll (rightward).
        let v_right = edge_scroll_velocity(viewport.max.x + 10.0, viewport);
        assert!((v_right - MAX_SCROLL_SPEED_PX_PER_S).abs() < 0.001);

        // Pointer half-way into the left dead-zone: half-speed leftward.
        let half_left = viewport.min.x + EDGE_DEAD_ZONE_PX * 0.5;
        let v_half_left = edge_scroll_velocity(half_left, viewport);
        assert!((v_half_left - -MAX_SCROLL_SPEED_PX_PER_S * 0.5).abs() < 0.001);

        // Pointer exactly at the dead-zone boundary: zero velocity.
        let boundary = viewport.min.x + EDGE_DEAD_ZONE_PX;
        assert_eq!(edge_scroll_velocity(boundary, viewport), 0.0);
    }
}
