use ab_glyph::{FontRef, PxScale};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;

use crate::model::Annotation;

const INTER_VARIABLE: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");

pub fn font() -> FontRef<'static> {
    FontRef::try_from_slice(INTER_VARIABLE).expect("bundled font parses")
}

/// Burn each annotation into `img`. Coordinates are in image pixel space.
pub fn burn(img: &mut RgbaImage, annotations: &[Annotation], font: &FontRef<'_>) {
    for a in annotations {
        match a {
            Annotation::Rect {
                x,
                y,
                w,
                h,
                stroke_color,
                stroke_width,
            } => draw_rect(img, *x, *y, *w, *h, *stroke_color, *stroke_width),
            Annotation::Text {
                x,
                y,
                text,
                font_size,
                color,
            } => draw_text(img, *x, *y, text, *font_size, *color, font),
        }
    }
}

fn draw_rect(
    img: &mut RgbaImage,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [u8; 4],
    stroke_width: f32,
) {
    let (iw, ih) = img.dimensions();
    let (nx, ny, nw, nh) = normalise_rect(x, y, w, h);
    if nw <= 0.0 || nh <= 0.0 {
        return;
    }
    // Fully outside the image? Skip.
    if nx + nw <= 0.0 || ny + nh <= 0.0 || nx >= iw as f32 || ny >= ih as f32 {
        return;
    }
    let rgba = Rgba(color);
    let thickness = stroke_width.round().max(1.0) as i32;
    for t in 0..thickness {
        let px = (nx + t as f32).round() as i32;
        let py = (ny + t as f32).round() as i32;
        let rw = (nw - 2.0 * t as f32).round() as i32;
        let rh = (nh - 2.0 * t as f32).round() as i32;
        if rw <= 0 || rh <= 0 {
            break;
        }
        let rect = Rect::at(px, py).of_size(rw as u32, rh as u32);
        // imageproc clips pixels outside the image bounds for us.
        draw_hollow_rect_mut(img, rect, rgba);
    }
}

fn normalise_rect(x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let nx = if w >= 0.0 { x } else { x + w };
    let ny = if h >= 0.0 { y } else { y + h };
    (nx, ny, w.abs(), h.abs())
}

fn draw_text(
    img: &mut RgbaImage,
    x: f32,
    y: f32,
    text: &str,
    font_size: f32,
    color: [u8; 4],
    font: &FontRef<'_>,
) {
    if text.is_empty() {
        return;
    }
    let scale = PxScale::from(font_size.max(1.0));
    draw_text_mut(
        img,
        Rgba(color),
        x.round() as i32,
        y.round() as i32,
        scale,
        font,
        text,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DEFAULT_STROKE_RGBA, DEFAULT_STROKE_WIDTH, DEFAULT_TEXT_RGBA};

    fn white_image(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]))
    }

    #[test]
    fn burn_rect_marks_pixels() {
        let mut img = white_image(100, 100);
        let font = font();
        burn(
            &mut img,
            &[Annotation::Rect {
                x: 10.0,
                y: 10.0,
                w: 30.0,
                h: 30.0,
                stroke_color: DEFAULT_STROKE_RGBA,
                stroke_width: DEFAULT_STROKE_WIDTH,
            }],
            &font,
        );
        // A pixel on the rect edge should now be the stroke color.
        let px = img.get_pixel(10, 20);
        assert_eq!(px.0, DEFAULT_STROKE_RGBA, "expected rect stroke on edge");
        // A pixel far from the rect should still be white.
        let px = img.get_pixel(80, 80);
        assert_eq!(px.0, [255, 255, 255, 255], "expected unchanged interior");
    }

    #[test]
    fn burn_rect_with_negative_size_is_normalised() {
        let mut img = white_image(100, 100);
        let font = font();
        burn(
            &mut img,
            &[Annotation::Rect {
                x: 50.0,
                y: 50.0,
                w: -20.0,
                h: -20.0,
                stroke_color: DEFAULT_STROKE_RGBA,
                stroke_width: 1.0,
            }],
            &font,
        );
        // Normalised rect covers (30..=49, 30..=49). Its right edge (x=49)
        // should carry stroke; a pixel well outside the rect stays white.
        assert_eq!(img.get_pixel(10, 10).0, [255, 255, 255, 255]);
        assert_eq!(img.get_pixel(49, 40).0, DEFAULT_STROKE_RGBA);
    }

    #[test]
    fn burn_text_marks_pixels() {
        let mut img = white_image(200, 60);
        let font = font();
        burn(
            &mut img,
            &[Annotation::Text {
                x: 10.0,
                y: 10.0,
                text: "hi".to_string(),
                font_size: 24.0,
                color: DEFAULT_TEXT_RGBA,
            }],
            &font,
        );
        // At least one pixel in the drawn glyph area should differ from white.
        let mut modified = 0usize;
        for y in 5..50 {
            for x in 5..80 {
                if img.get_pixel(x, y).0 != [255, 255, 255, 255] {
                    modified += 1;
                }
            }
        }
        assert!(
            modified > 5,
            "expected text pixels to be drawn; got {modified}"
        );
    }

    #[test]
    fn burn_ignores_out_of_bounds_rect() {
        let mut img = white_image(50, 50);
        let font = font();
        burn(
            &mut img,
            &[Annotation::Rect {
                x: 60.0,
                y: 60.0,
                w: 100.0,
                h: 100.0,
                stroke_color: DEFAULT_STROKE_RGBA,
                stroke_width: 2.0,
            }],
            &font,
        );
        // Nothing should be modified inside the 50x50 image.
        for px in img.pixels() {
            assert_eq!(px.0, [255, 255, 255, 255]);
        }
    }
}
