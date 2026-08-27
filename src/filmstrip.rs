use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, ScrollArea, Sense, Stroke,
    Ui, Vec2,
};

use crate::theme;
use crate::thumbs::ThumbCache;

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
}

impl Default for FilmstripGeometry {
    fn default() -> Self {
        Self {
            thumb_w: 80.0,
            thumb_h: 112.0,
            gap: 4.0,
            top_pad: 4.0,
            label_h: 14.0,
            label_every_n: 5,
            prefetch_pad: 6,
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
        let first = (x0 / pitch).floor().max(0.0) as usize;
        let last_seen = ((x1 - self.gap) / pitch).ceil() as isize;
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
        let x = container_origin.x + index as f32 * self.pitch();
        let y = container_origin.y + self.top_pad;
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(self.thumb_w, self.thumb_h))
    }
}

/// Output of one draw pass of the filmstrip.
#[derive(Debug, Default, Clone, Copy)]
pub struct FilmstripAction {
    /// The user clicked or dragged onto this frame; caller should seek to it.
    pub seek_to: Option<usize>,
    /// The user changed `current_frame` before this frame; scroll it into view.
    pub scroll_into_view: bool,
}

pub struct FilmstripDrawParams<'a> {
    pub geom: FilmstripGeometry,
    pub total_frames: usize,
    pub current_frame: usize,
    pub prev_current_frame: usize,
    pub thumbs: &'a mut ThumbCache,
}

/// Draw the filmstrip inside `ui`, returning a request for the caller to seek
/// somewhere. The strip fills the width and its height is `geom.row_height()`.
pub fn draw(ui: &mut Ui, params: FilmstripDrawParams<'_>) -> FilmstripAction {
    let FilmstripDrawParams {
        geom,
        total_frames,
        current_frame,
        prev_current_frame,
        thumbs,
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

    let content_width = geom.total_width(total_frames);
    let row_h = geom.row_height();
    let want_scroll = current_frame != prev_current_frame;

    ScrollArea::horizontal()
        .id_salt("frammpeg-filmstrip")
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            let (content_rect, response) = ui.allocate_exact_size(
                Vec2::new(content_width.max(viewport.width()), row_h),
                Sense::click_and_drag(),
            );
            let painter = ui.painter_at(content_rect);
            painter.rect_filled(content_rect, CornerRadius::ZERO, theme::PANEL);

            let x0 = viewport.min.x - content_rect.min.x;
            let x1 = viewport.max.x - content_rect.min.x;
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
                );
            }

            // Auto-scroll to keep `current_frame` in view when it just changed.
            // Skip when the pointer is dragging — that's the user scrubbing, and
            // seeking-to-drag already keeps them centered by definition.
            if want_scroll {
                if let Some((lo, hi)) = visible {
                    let edge_pad: usize = 1;
                    let lo_edge = lo.saturating_add(edge_pad).min(hi);
                    let hi_edge = hi.saturating_sub(edge_pad).max(lo);
                    if current_frame < lo_edge || current_frame > hi_edge {
                        let rect = geom.thumb_rect(content_rect.min, current_frame);
                        ui.scroll_to_rect(rect, Some(Align::Center));
                        action.scroll_into_view = true;
                    }
                } else {
                    let rect = geom.thumb_rect(content_rect.min, current_frame);
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    action.scroll_into_view = true;
                }
            }

            handle_interaction(&response, content_rect, geom, total_frames, &mut action);
        });

    action
}

fn paint_thumbs(
    painter: &egui::Painter,
    origin: Pos2,
    geom: FilmstripGeometry,
    thumbs: &mut ThumbCache,
    range: (usize, usize),
    current_frame: usize,
) {
    let (lo, hi) = range;
    for idx in lo..=hi {
        let rect = geom.thumb_rect(origin, idx);
        let selected = idx == current_frame;
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

fn handle_interaction(
    response: &Response,
    content_rect: Rect,
    geom: FilmstripGeometry,
    total_frames: usize,
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
            action.seek_to = Some(idx.min(last));
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
        let g = geom(); // pitch = 84
        let (lo, hi) = g.visible_range(0.0, 200.0, 100).unwrap();
        assert_eq!(lo, 0);
        // 200 / 84 ~ 2.38 -> ceil = 3, so at least index 2 is visible.
        assert!(hi >= 2);
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
        let g = geom(); // pitch = 84
        let (lo, hi) = g.visible_range(200.0, 400.0, 100).unwrap();
        // 200 / 84 ~ 2.38 -> floor 2
        assert_eq!(lo, 2);
        assert!(hi >= 4);
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
        // 10 * 84 - 4 = 836
        assert!((w - 836.0).abs() < 0.001);
    }

    #[test]
    fn total_width_zero_frames() {
        let g = geom();
        assert_eq!(g.total_width(0), 0.0);
    }
}
