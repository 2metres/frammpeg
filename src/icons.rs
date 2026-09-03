use std::collections::HashMap;

use egui::{Button, Color32, ColorImage, Response, TextureHandle, TextureOptions, Ui, Vec2};

const SKIP_BACK: &[u8] = include_bytes!("../assets/icons/skip-back.svg");
const CHEVRONS_LEFT: &[u8] = include_bytes!("../assets/icons/chevrons-left.svg");
const CHEVRON_LEFT: &[u8] = include_bytes!("../assets/icons/chevron-left.svg");
const PLAY: &[u8] = include_bytes!("../assets/icons/play.svg");
const PAUSE: &[u8] = include_bytes!("../assets/icons/pause.svg");
const CHEVRON_RIGHT: &[u8] = include_bytes!("../assets/icons/chevron-right.svg");
const CHEVRONS_RIGHT: &[u8] = include_bytes!("../assets/icons/chevrons-right.svg");
const SKIP_FORWARD: &[u8] = include_bytes!("../assets/icons/skip-forward.svg");
const SQUARE: &[u8] = include_bytes!("../assets/icons/square.svg");
const TYPE: &[u8] = include_bytes!("../assets/icons/type.svg");
const SCISSORS: &[u8] = include_bytes!("../assets/icons/scissors.svg");
const BOOKMARK: &[u8] = include_bytes!("../assets/icons/bookmark.svg");
const X: &[u8] = include_bytes!("../assets/icons/x.svg");
const TRASH: &[u8] = include_bytes!("../assets/icons/trash.svg");
const ROTATE_CCW: &[u8] = include_bytes!("../assets/icons/rotate-ccw.svg");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    SkipBack,
    #[allow(dead_code)]
    StepBack10,
    StepBack,
    Play,
    Pause,
    StepForward,
    #[allow(dead_code)]
    StepForward10,
    SkipForward,
    Rectangle,
    Text,
    Scissors,
    Bookmark,
    X,
    Trash,
    RotateCcw,
}

impl Icon {
    fn svg_bytes(self) -> &'static [u8] {
        match self {
            Icon::SkipBack => SKIP_BACK,
            Icon::StepBack10 => CHEVRONS_LEFT,
            Icon::StepBack => CHEVRON_LEFT,
            Icon::Play => PLAY,
            Icon::Pause => PAUSE,
            Icon::StepForward => CHEVRON_RIGHT,
            Icon::StepForward10 => CHEVRONS_RIGHT,
            Icon::SkipForward => SKIP_FORWARD,
            Icon::Rectangle => SQUARE,
            Icon::Text => TYPE,
            Icon::Scissors => SCISSORS,
            Icon::Bookmark => BOOKMARK,
            Icon::X => X,
            Icon::Trash => TRASH,
            Icon::RotateCcw => ROTATE_CCW,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Icon::SkipBack => "skip-back",
            Icon::StepBack10 => "chevrons-left",
            Icon::StepBack => "chevron-left",
            Icon::Play => "play",
            Icon::Pause => "pause",
            Icon::StepForward => "chevron-right",
            Icon::StepForward10 => "chevrons-right",
            Icon::SkipForward => "skip-forward",
            Icon::Rectangle => "square",
            Icon::Text => "type",
            Icon::Scissors => "scissors",
            Icon::Bookmark => "bookmark",
            Icon::X => "x",
            Icon::Trash => "trash",
            Icon::RotateCcw => "rotate-ccw",
        }
    }
}

pub type CacheKey = (Icon, u32, [u8; 4]);

#[derive(Default)]
pub struct IconCache {
    entries: HashMap<CacheKey, TextureHandle>,
}

impl IconCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn cache_key(icon: Icon, size_px: u32, color: Color32) -> CacheKey {
        (icon, size_px, [color.r(), color.g(), color.b(), color.a()])
    }

    fn texture(
        &mut self,
        ctx: &egui::Context,
        icon: Icon,
        size_pt: f32,
        color: Color32,
    ) -> Option<TextureHandle> {
        let ppp = ctx.pixels_per_point().max(1.0);
        let size_px = (size_pt * ppp).round().max(1.0) as u32;
        let key = Self::cache_key(icon, size_px, color);
        if let Some(tex) = self.entries.get(&key) {
            return Some(tex.clone());
        }
        let tex = rasterize(ctx, icon, size_px, color)?;
        self.entries.insert(key, tex.clone());
        Some(tex)
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        icon: Icon,
        size_pt: f32,
        color: Color32,
        min_size: Vec2,
        frame: bool,
    ) -> Response {
        let ctx = ui.ctx().clone();
        match self.texture(&ctx, icon, size_pt, color) {
            Some(tex) => {
                let image = egui::Image::from_texture(&tex).fit_to_exact_size(Vec2::splat(size_pt));
                ui.add(Button::image(image).min_size(min_size).frame(frame))
            }
            None => ui.add(Button::new(icon.name()).min_size(min_size).frame(frame)),
        }
    }
}

fn recolor_svg(bytes: &[u8], color: Color32) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let hex = format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b());
    // Lucide icons use stroke="currentColor" consistently; swap to a concrete
    // hex color so usvg (without CSS) resolves the stroke paint. Fill stays
    // "none" so we don't accidentally fill outline glyphs.
    Some(text.replace("currentColor", &hex))
}

fn rasterize(
    ctx: &egui::Context,
    icon: Icon,
    size_px: u32,
    color: Color32,
) -> Option<TextureHandle> {
    let recolored = recolor_svg(icon.svg_bytes(), color)?;
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(&recolored, &opt).ok()?;
    let source = tree.size();
    let scale = (size_px as f32) / source.width().max(1.0);
    let mut pixmap = tiny_skia::Pixmap::new(size_px, size_px)?;
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let image =
        ColorImage::from_rgba_premultiplied([size_px as usize, size_px as usize], pixmap.data());
    Some(ctx.load_texture(
        format!(
            "frammpeg-icon-{}-{}x-{:02x}{:02x}{:02x}{:02x}",
            icon.name(),
            size_px,
            color.r(),
            color.g(),
            color.b(),
            color.a()
        ),
        image,
        TextureOptions::LINEAR,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recolor_swaps_current_color_for_hex() {
        let svg = br#"<svg stroke="currentColor"><path d="M0 0" /></svg>"#;
        let out = recolor_svg(svg, Color32::from_rgb(0xAB, 0xCD, 0xEF)).unwrap();
        assert!(out.contains("stroke=\"#abcdef\""));
        assert!(!out.contains("currentColor"));
    }

    #[test]
    fn recolor_no_op_when_no_current_color() {
        let svg = br##"<svg stroke="#000"><path d="M0 0" /></svg>"##;
        let out = recolor_svg(svg, Color32::WHITE).unwrap();
        assert_eq!(out, r##"<svg stroke="#000"><path d="M0 0" /></svg>"##);
    }

    #[test]
    fn cache_key_distinguishes_size_and_color() {
        let a = IconCache::cache_key(Icon::Play, 24, Color32::WHITE);
        let b = IconCache::cache_key(Icon::Play, 48, Color32::WHITE);
        let c = IconCache::cache_key(Icon::Play, 24, Color32::BLACK);
        let d = IconCache::cache_key(Icon::Pause, 24, Color32::WHITE);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn cache_key_equal_for_same_inputs() {
        let a = IconCache::cache_key(Icon::Play, 24, Color32::from_rgb(0x4E, 0xC9, 0xB0));
        let b = IconCache::cache_key(Icon::Play, 24, Color32::from_rgb(0x4E, 0xC9, 0xB0));
        assert_eq!(a, b);
    }

    #[test]
    fn texture_cache_reuses_handle_for_same_key() {
        let ctx = egui::Context::default();
        let mut cache = IconCache::new();
        let a = cache
            .texture(&ctx, Icon::Play, 24.0, Color32::from_rgb(1, 2, 3))
            .expect("first raster");
        assert_eq!(cache.entries.len(), 1);
        let b = cache
            .texture(&ctx, Icon::Play, 24.0, Color32::from_rgb(1, 2, 3))
            .expect("cache hit");
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(a.id(), b.id());
    }

    #[test]
    fn texture_cache_distinguishes_color_and_size() {
        let ctx = egui::Context::default();
        let mut cache = IconCache::new();
        let _a = cache
            .texture(&ctx, Icon::Play, 24.0, Color32::WHITE)
            .expect("raster a");
        let _b = cache
            .texture(&ctx, Icon::Play, 48.0, Color32::WHITE)
            .expect("raster b");
        let _c = cache
            .texture(&ctx, Icon::Play, 24.0, Color32::BLACK)
            .expect("raster c");
        assert_eq!(cache.entries.len(), 3);
    }

    #[test]
    fn recolor_round_trip_parses_as_valid_svg() {
        // Sanity: after recolor, usvg still parses every embedded icon.
        let icons = [
            Icon::SkipBack,
            Icon::StepBack10,
            Icon::StepBack,
            Icon::Play,
            Icon::Pause,
            Icon::StepForward,
            Icon::StepForward10,
            Icon::SkipForward,
            Icon::Rectangle,
            Icon::Text,
            Icon::Scissors,
            Icon::Bookmark,
            Icon::X,
            Icon::Trash,
            Icon::RotateCcw,
        ];
        let opt = usvg::Options::default();
        for icon in icons {
            let recolored =
                recolor_svg(icon.svg_bytes(), Color32::from_rgb(1, 2, 3)).expect("valid utf-8");
            let tree = usvg::Tree::from_str(&recolored, &opt)
                .unwrap_or_else(|e| panic!("parse {:?}: {}", icon, e));
            assert!(tree.size().width() > 0.0);
            assert!(tree.size().height() > 0.0);
        }
    }
}
