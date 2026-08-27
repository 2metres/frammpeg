use egui::{Color32, CornerRadius, Stroke, Visuals};

// Neutral dark palette. Near-black canvas, cool mid-grays for chrome, one
// muted teal accent (borrowed from VS Code's type-highlight hue — recognisable,
// professional, plays with debugging content without shouting at it).
pub const CANVAS: Color32 = Color32::from_rgb(0x0B, 0x0D, 0x10);
pub const PANEL: Color32 = Color32::from_rgb(0x14, 0x17, 0x1B);
pub const WINDOW: Color32 = Color32::from_rgb(0x1A, 0x1E, 0x23);
pub const WIDGET_IDLE: Color32 = Color32::from_rgb(0x1E, 0x23, 0x29);
pub const WIDGET_HOVERED: Color32 = Color32::from_rgb(0x26, 0x2C, 0x34);
pub const WIDGET_ACTIVE: Color32 = Color32::from_rgb(0x2E, 0x35, 0x3E);
pub const STROKE: Color32 = Color32::from_rgb(0x2A, 0x30, 0x38);
pub const STROKE_STRONG: Color32 = Color32::from_rgb(0x3A, 0x42, 0x4C);
pub const TEXT: Color32 = Color32::from_rgb(0xD8, 0xDC, 0xE1);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x8A, 0x92, 0x9B);
pub const ACCENT: Color32 = Color32::from_rgb(0x4E, 0xC9, 0xB0);
pub const ACCENT_HOVERED: Color32 = Color32::from_rgb(0x6A, 0xD6, 0xC1);

pub fn visuals() -> Visuals {
    let mut v = Visuals::dark();

    v.panel_fill = PANEL;
    v.window_fill = WINDOW;
    v.extreme_bg_color = CANVAS;
    v.faint_bg_color = WIDGET_IDLE;
    v.code_bg_color = WIDGET_IDLE;

    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    let rounding = CornerRadius::same(4);

    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, STROKE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
    v.widgets.noninteractive.corner_radius = rounding;

    v.widgets.inactive.bg_fill = WIDGET_IDLE;
    v.widgets.inactive.weak_bg_fill = WIDGET_IDLE;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, STROKE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = rounding;

    v.widgets.hovered.bg_fill = WIDGET_HOVERED;
    v.widgets.hovered.weak_bg_fill = WIDGET_HOVERED;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, STROKE_STRONG);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.corner_radius = rounding;

    v.widgets.active.bg_fill = WIDGET_ACTIVE;
    v.widgets.active.weak_bg_fill = WIDGET_ACTIVE;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.corner_radius = rounding;

    v.widgets.open.bg_fill = WIDGET_ACTIVE;
    v.widgets.open.weak_bg_fill = WIDGET_ACTIVE;
    v.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_HOVERED);
    v.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.open.corner_radius = rounding;

    v.window_stroke = Stroke::new(1.0, STROKE);
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(6);

    v
}
