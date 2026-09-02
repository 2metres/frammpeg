use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, ScrollArea, Sense, Stroke,
    Ui, Vec2,
};

use crate::theme;
use crate::thumbs::ThumbCache;

pub const TRIM_HANDLE_W: f32 = 22.0;
pub const TRIM_RAIL_H: f32 = 4.0;

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

    /// Pointer-x within the content coord system (already relative to origin)
    /// mapped to a frame index, snapped to whichever thumb the pointer is over
    /// (nearest boundary for gap positions).
    pub fn x_to_frame(&self, x_content: f32, total_frames: usize) -> usize {
        if total_frames == 0 {
            return 0;
        }
        let last = total_frames - 1;
        let pitch = self.pitch().max(1.0);
        let x_adj = (x_content - self.left_pad).max(0.0);
        let idx = (x_adj / pitch).floor().max(0.0) as usize;
        idx.min(last)
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
    /// Use instant (non-animated) scroll for large jumps (Home/End).
    pub instant_scroll: bool,
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
        instant_scroll,
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

    let content_width = geom.total_width(total_frames);
    let row_h = geom.row_height();
    let want_scroll = current_frame != prev_current_frame;

    ScrollArea::horizontal()
        .id_salt("frammpeg-filmstrip")
        .auto_shrink([false, false])
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
            let visible = geom.visible_range(x0, x1, total_frames);
            let prefetch = geom.prefetch_range(visible, total_frames);

            // Kick off decodes for prefetch range that we haven't seen yet.
            if let Some((lo, hi)) = prefetch {
                for idx in lo..=hi {
                    thumbs.request(idx);
                }
            }

            if let Some((lo, hi)) = visible {
                paint_thumbs(
                    &painter,
                    content_rect.min,
                    geom,
                    thumbs,
                    (lo, hi),
                    current_frame,
                    (*trim_start, *trim_end),
                    trim_enabled,
                );
            }

            if trim_enabled {
                paint_trim_rails(&painter, content_rect.min, geom, (*trim_start, *trim_end));
            }

            // Keep the current frame centered whenever it changes — during play
            // the filmstrip scrolls past under a fixed center indicator; during
            // manual navigation the target frame slides to the middle.
            if want_scroll {
                let rect = geom.thumb_rect(content_rect.min, current_frame);
                if instant_scroll {
                    ui.scroll_to_rect_animation(
                        rect,
                        Some(Align::Center),
                        egui::style::ScrollAnimation::none(),
                    );
                } else {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                }
                action.scroll_into_view = true;
            }

            let mut handle_pointer_captured = false;
            if trim_enabled {
                handle_pointer_captured = handle_trim_interaction(
                    ui,
                    content_rect,
                    geom,
                    total_frames,
                    trim_start,
                    trim_end,
                    &mut action,
                );
            }

            if !handle_pointer_captured {
                handle_seek_interaction(
                    &seek_response,
                    content_rect,
                    geom,
                    total_frames,
                    (*trim_start, *trim_end),
                    trim_enabled,
                    &mut action,
                );
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
) {
    let (lo, hi) = range;
    let (trim_start, trim_end) = trim;
    for idx in lo..=hi {
        let rect = geom.thumb_rect(origin, idx);
        let selected = idx == current_frame;
        let in_trim = !trim_enabled || (idx >= trim_start && idx <= trim_end);
        let bg = if selected {
            theme::WIDGET_ACTIVE
        } else {
            theme::WIDGET_IDLE
        };
        painter.rect_filled(rect, CornerRadius::same(2), bg);

        if let Some(tex) = thumbs.get(idx) {
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

        if (idx + 1) % geom.label_every_n.max(1) == 0 || idx == current_frame {
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
                format!("{}", idx + 1),
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
    geom: FilmstripGeometry,
    total_frames: usize,
    trim_start: &mut usize,
    trim_end: &mut usize,
    action: &mut FilmstripAction,
) -> bool {
    let last = total_frames.saturating_sub(1);
    let start_rect = geom.trim_handle_rect(content_rect.min, *trim_start, TrimHandle::Start);
    let end_rect = geom.trim_handle_rect(content_rect.min, *trim_end, TrimHandle::End);

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
            let x = pos.x - content_rect.min.x;
            let idx = geom.x_to_frame(x, total_frames);
            let clamped_hi = trim_end.saturating_sub(1);
            *trim_start = idx.min(clamped_hi);
        }
    }
    if end_resp.dragged() {
        if let Some(pos) = end_resp.interact_pointer_pos() {
            let x = pos.x - content_rect.min.x;
            let idx = geom.x_to_frame(x, total_frames);
            let clamped_lo = trim_start.saturating_add(1);
            *trim_end = idx.max(clamped_lo).min(last);
        }
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

fn handle_seek_interaction(
    response: &Response,
    content_rect: Rect,
    geom: FilmstripGeometry,
    total_frames: usize,
    trim: (usize, usize),
    trim_enabled: bool,
    action: &mut FilmstripAction,
) {
    if total_frames == 0 {
        return;
    }
    let pitch = geom.pitch();
    if pitch <= 0.0 {
        return;
    }
    let last = total_frames - 1;
    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let x = (pos.x - content_rect.min.x) / pitch;
            let idx = (x.floor() as isize).max(0) as usize;
            let clamped = idx.min(last);
            let target = if trim_enabled {
                clamped.clamp(trim.0, trim.1)
            } else {
                clamped
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
    fn x_to_frame_snaps_to_thumb_at_start() {
        let g = geom();
        assert_eq!(g.x_to_frame(0.0, 100), 0);
        // Pointer just inside thumb 3 (3 * pitch = 300, +5 -> still index 3).
        assert_eq!(g.x_to_frame(3.0 * g.pitch() + 5.0, 100), 3);
    }

    #[test]
    fn x_to_frame_clamps_past_end() {
        let g = geom();
        assert_eq!(g.x_to_frame(1e6, 10), 9);
    }

    #[test]
    fn x_to_frame_empty_video() {
        let g = geom();
        assert_eq!(g.x_to_frame(100.0, 0), 0);
    }
}
