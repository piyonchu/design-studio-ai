//! Deterministic, model-free image edits (Pro pipeline B1). Each op transforms
//! raw bytes → raw bytes using the `image` crate; the route records the result
//! as a new version (A2), so edits are **free, instant, and non-destructive** —
//! the original is always a prior version you can roll back to.

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat};
use serde::Deserialize;

use crate::error::AppError;

/// One edit operation + its parameters. Tagged by `op` in the request body.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOp {
    Crop { x: u32, y: u32, w: u32, h: u32 },
    Resize { w: u32, h: u32 },
    Flip { axis: Axis },
    Rotate { degrees: u32 },
    Grayscale,
    Invert,
    /// Hue rotate (recolor / palette shift), degrees -180..=180.
    Hue { degrees: i32 },
    Brighten { value: i32 },
    /// Knock out a flat background to transparency (samples the top-left pixel).
    RemoveBg {
        #[serde(default = "default_tolerance")]
        tolerance: u8,
    },
    /// Re-encode to another format ("png" | "jpeg").
    Convert { format: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}

fn default_tolerance() -> u8 {
    24
}

const MAX_DIM: u32 = 4096;

impl EditOp {
    /// A human-readable change note for the version timeline.
    pub fn note(&self) -> String {
        match self {
            EditOp::Crop { w, h, .. } => format!("Cropped to {w}×{h}"),
            EditOp::Resize { w, h } => format!("Resized to {w}×{h}"),
            EditOp::Flip { axis: Axis::Horizontal } => "Flipped horizontally".into(),
            EditOp::Flip { axis: Axis::Vertical } => "Flipped vertically".into(),
            EditOp::Rotate { degrees } => format!("Rotated {degrees}°"),
            EditOp::Grayscale => "Grayscale".into(),
            EditOp::Invert => "Inverted colors".into(),
            EditOp::Hue { degrees } => format!("Hue shifted {degrees:+}°"),
            EditOp::Brighten { value } => format!("Brightness {value:+}"),
            EditOp::RemoveBg { .. } => "Background removed".into(),
            EditOp::Convert { format } => format!("Converted to {format}"),
        }
    }
}

/// Apply an op to raster bytes, returning the new bytes + their mime type.
/// `BadRequest` for invalid params (out-of-bounds crop, bad format, …).
pub fn apply(bytes: &[u8], mime: &str, op: &EditOp) -> Result<(Vec<u8>, String), AppError> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| AppError::BadRequest(format!("cannot decode image: {e}")))?;

    // Output format defaults to the input's; `convert` overrides; `remove_bg`
    // forces PNG (it introduces an alpha channel).
    let mut out_fmt = if mime.contains("jpeg") || mime.contains("jpg") {
        ImageFormat::Jpeg
    } else {
        ImageFormat::Png
    };

    let edited: DynamicImage = match op {
        EditOp::Crop { x, y, w, h } => {
            let (iw, ih) = (img.width(), img.height());
            if *w == 0 || *h == 0 || x + w > iw || y + h > ih {
                return Err(AppError::BadRequest("crop rectangle out of bounds".into()));
            }
            img.crop_imm(*x, *y, *w, *h)
        }
        EditOp::Resize { w, h } => {
            if *w == 0 || *h == 0 || *w > MAX_DIM || *h > MAX_DIM {
                return Err(AppError::BadRequest(format!("size must be 1..={MAX_DIM}")));
            }
            img.resize_exact(*w, *h, image::imageops::FilterType::Lanczos3)
        }
        EditOp::Flip { axis: Axis::Horizontal } => img.fliph(),
        EditOp::Flip { axis: Axis::Vertical } => img.flipv(),
        EditOp::Rotate { degrees } => match degrees {
            90 => img.rotate90(),
            180 => img.rotate180(),
            270 => img.rotate270(),
            _ => return Err(AppError::BadRequest("rotate degrees must be 90, 180, or 270".into())),
        },
        EditOp::Grayscale => img.grayscale(),
        EditOp::Invert => {
            let mut i = img;
            i.invert();
            i
        }
        EditOp::Hue { degrees } => img.huerotate(*degrees),
        EditOp::Brighten { value } => img.brighten(*value),
        EditOp::RemoveBg { tolerance } => {
            out_fmt = ImageFormat::Png;
            remove_background(&img, *tolerance)
        }
        EditOp::Convert { format } => {
            out_fmt = parse_format(format)?;
            img
        }
    };

    // JPEG can't carry alpha — flatten to RGB before encoding.
    let to_encode = if out_fmt == ImageFormat::Jpeg {
        DynamicImage::ImageRgb8(edited.to_rgb8())
    } else {
        edited
    };

    let mut buf = Cursor::new(Vec::new());
    to_encode
        .write_to(&mut buf, out_fmt)
        .map_err(|e| AppError::Internal(format!("encode failed: {e}")))?;
    let out_mime = if out_fmt == ImageFormat::Jpeg { "image/jpeg" } else { "image/png" };
    Ok((buf.into_inner(), out_mime.to_string()))
}

/// Knock out a flat background: anything within `tolerance` (per-channel L1) of
/// the top-left pixel becomes transparent. Good enough for clean cutouts (the
/// asset style this product targets); a learned matte can replace it later.
fn remove_background(img: &DynamicImage, tolerance: u8) -> DynamicImage {
    let mut rgba = img.to_rgba8();
    let bg = *rgba.get_pixel(0, 0);
    let t = tolerance as i32 * 3;
    for p in rgba.pixels_mut() {
        let d = (p[0] as i32 - bg[0] as i32).abs()
            + (p[1] as i32 - bg[1] as i32).abs()
            + (p[2] as i32 - bg[2] as i32).abs();
        if d <= t {
            p[3] = 0;
        }
    }
    DynamicImage::ImageRgba8(rgba)
}

fn parse_format(format: &str) -> Result<ImageFormat, AppError> {
    match format.to_ascii_lowercase().as_str() {
        "png" => Ok(ImageFormat::Png),
        "jpeg" | "jpg" => Ok(ImageFormat::Jpeg),
        other => Err(AppError::BadRequest(format!("unsupported format '{other}' (png|jpeg)"))),
    }
}

// ── Masked deterministic recolor (the "change its colour" edit intent) ───────
//
// Diffusion inpaint is the wrong tool for a pure colour change: the model
// regenerates "what's plausible here" and the surrounding context + the
// object's colour prior fight the request (a banana wants to be yellow). This
// instead *keeps every pixel's shading and texture* and swaps only hue/
// saturation toward the target — exact, instant, free.

/// Recolor the masked region of `base` toward `target` (`#rrggbb`), preserving
/// per-pixel luminance (shading, highlights, outlines survive). `mask`: painted
/// = opaque + bright pixels, same convention as inpaint. Returns PNG bytes.
pub fn recolor_masked(base: &[u8], mask: &[u8], target: &str) -> Result<Vec<u8>, AppError> {
    let (tr, tg, tb) = parse_hex(target)?;
    let (th, ts, _) = rgb_to_hsl(tr, tg, tb);

    let base_img = image::load_from_memory(base)
        .map_err(|e| AppError::BadRequest(format!("cannot decode image: {e}")))?;
    let (w, h) = (base_img.width(), base_img.height());
    let mut out = base_img.to_rgba8();

    let mask_img = image::load_from_memory(mask)
        .map_err(|e| AppError::BadRequest(format!("cannot decode mask image: {e}")))?;
    let mask_rgba = if mask_img.dimensions() == (w, h) {
        mask_img.to_rgba8()
    } else {
        image::imageops::resize(&mask_img.to_rgba8(), w, h, image::imageops::FilterType::Nearest)
    };

    let mut touched = 0u64;
    for y in 0..h {
        for x in 0..w {
            let m = mask_rgba.get_pixel(x, y);
            let lum = 0.299 * m[0] as f32 + 0.587 * m[1] as f32 + 0.114 * m[2] as f32;
            if m[3] > 32 && lum > 96.0 {
                let p = out.get_pixel_mut(x, y);
                if p[3] == 0 {
                    continue; // fully transparent pixels have no colour to change
                }
                let (_, s, l) = rgb_to_hsl(p[0], p[1], p[2]);
                // Target hue outright; pull saturation toward the target's so a
                // grey region can *become* coloured, but shading (L) is kept.
                let ns = s + (ts - s) * 0.85;
                let (r, g, b) = hsl_to_rgb(th, ns, l);
                (p[0], p[1], p[2]) = (r, g, b);
                touched += 1;
            }
        }
    }
    if touched == 0 {
        return Err(AppError::BadRequest("mask is empty — paint the region to recolor".into()));
    }

    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("encode failed: {e}")))?;
    Ok(buf.into_inner())
}

/// Replace the masked region's pixels with the surrounding colour before a
/// diffusion inpaint ("masked content: fill"). The Fooocus inpaint head feeds
/// the *original* pixels to the model to harmonize the fill — which anchors the
/// output to the object being changed (ask for a red apple over a yellow
/// banana and it re-renders the banana). Hiding the region's content lets the
/// prompt drive what appears. Verified live: with original pixels the model
/// reproduced the masked object; with fill it followed the prompt.
pub fn neutralize_masked(base: &[u8], mask: &[u8]) -> Result<Vec<u8>, AppError> {
    let base_img = image::load_from_memory(base)
        .map_err(|e| AppError::BadRequest(format!("cannot decode image: {e}")))?;
    let (w, h) = (base_img.width(), base_img.height());
    let mut out = base_img.to_rgba8();

    let mask_img = image::load_from_memory(mask)
        .map_err(|e| AppError::BadRequest(format!("cannot decode mask image: {e}")))?;
    let mask_rgba = if mask_img.dimensions() == (w, h) {
        mask_img.to_rgba8()
    } else {
        image::imageops::resize(&mask_img.to_rgba8(), w, h, image::imageops::FilterType::Nearest)
    };
    let painted = |x: u32, y: u32| {
        let m = mask_rgba.get_pixel(x, y);
        let lum = 0.299 * m[0] as f32 + 0.587 * m[1] as f32 + 0.114 * m[2] as f32;
        m[3] > 32 && lum > 96.0
    };

    // Mask bounding box, then the mean colour of the UNMASKED pixels in a ring
    // around it — the local surroundings the fill should blend into.
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    for y in 0..h {
        for x in 0..w {
            if painted(x, y) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x0 > x1 {
        return Err(AppError::BadRequest("mask is empty — paint the region to edit".into()));
    }
    const RING: u32 = 32;
    let (rx0, ry0) = (x0.saturating_sub(RING), y0.saturating_sub(RING));
    let (rx1, ry1) = ((x1 + RING).min(w - 1), (y1 + RING).min(h - 1));
    let (mut sr, mut sg, mut sb, mut n) = (0u64, 0u64, 0u64, 0u64);
    for y in ry0..=ry1 {
        for x in rx0..=rx1 {
            if !painted(x, y) {
                let p = out.get_pixel(x, y);
                if p[3] > 0 {
                    sr += p[0] as u64;
                    sg += p[1] as u64;
                    sb += p[2] as u64;
                    n += 1;
                }
            }
        }
    }
    let fill = if n > 0 {
        [(sr / n) as u8, (sg / n) as u8, (sb / n) as u8, 255]
    } else {
        [128, 128, 128, 255]
    };
    for y in 0..h {
        for x in 0..w {
            if painted(x, y) {
                out.put_pixel(x, y, image::Rgba(fill));
            }
        }
    }

    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("encode failed: {e}")))?;
    Ok(buf.into_inner())
}

/// `#rrggbb` (or `rrggbb`) → (r, g, b).
fn parse_hex(s: &str) -> Result<(u8, u8, u8), AppError> {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::BadRequest(format!("invalid colour '{s}' (want #rrggbb)")));
    }
    let v = u32::from_str_radix(hex, 16).expect("validated hex");
    Ok(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// RGB (0–255) → HSL (h in 0–360, s/l in 0–1).
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - r).abs() < f32::EPSILON {
        60.0 * (((g - b) / d) % 6.0)
    } else if (max - g).abs() < f32::EPSILON {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (if h < 0.0 { h + 360.0 } else { h }, s, l)
}

/// HSL (h 0–360, s/l 0–1) → RGB (0–255).
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny 4×2 red PNG (with a distinct corner) to exercise the ops.
    fn sample_png() -> Vec<u8> {
        let mut img = image::RgbaImage::new(4, 2);
        for p in img.pixels_mut() {
            *p = image::Rgba([200, 30, 30, 255]);
        }
        img.put_pixel(0, 0, image::Rgba([10, 10, 10, 255])); // a non-bg corner
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn dims(bytes: &[u8]) -> (u32, u32) {
        let i = image::load_from_memory(bytes).unwrap();
        (i.width(), i.height())
    }

    #[test]
    fn resize_changes_dimensions() {
        let (out, mime) = apply(&sample_png(), "image/png", &EditOp::Resize { w: 8, h: 8 }).unwrap();
        assert_eq!(dims(&out), (8, 8));
        assert_eq!(mime, "image/png");
    }

    #[test]
    fn rotate_90_swaps_dimensions() {
        let (out, _) = apply(&sample_png(), "image/png", &EditOp::Rotate { degrees: 90 }).unwrap();
        assert_eq!(dims(&out), (2, 4));
    }

    #[test]
    fn crop_out_of_bounds_is_rejected() {
        let err = apply(&sample_png(), "image/png", &EditOp::Crop { x: 0, y: 0, w: 99, h: 99 });
        assert!(matches!(err, Err(AppError::BadRequest(_))));
    }

    #[test]
    fn bad_rotation_and_format_are_rejected() {
        assert!(apply(&sample_png(), "image/png", &EditOp::Rotate { degrees: 45 }).is_err());
        assert!(apply(&sample_png(), "image/png", &EditOp::Convert { format: "gif".into() }).is_err());
    }

    #[test]
    fn convert_to_jpeg_drops_alpha_and_changes_mime() {
        let (out, mime) =
            apply(&sample_png(), "image/png", &EditOp::Convert { format: "jpeg".into() }).unwrap();
        assert_eq!(mime, "image/jpeg");
        // Re-decodes as a valid jpeg.
        assert_eq!(dims(&out), (4, 2));
    }

    #[test]
    fn remove_bg_makes_background_transparent() {
        // Fill the whole image with one bg color so the corner sample matches.
        let mut img = image::RgbaImage::from_pixel(3, 3, image::Rgba([255, 255, 255, 255]));
        img.put_pixel(1, 1, image::Rgba([0, 0, 0, 255])); // foreground pixel
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut buf, ImageFormat::Png).unwrap();

        let (out, mime) = apply(&buf.into_inner(), "image/png", &EditOp::RemoveBg { tolerance: 10 }).unwrap();
        assert_eq!(mime, "image/png");
        let result = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(result.get_pixel(0, 0)[3], 0, "background corner is transparent");
        assert_eq!(result.get_pixel(1, 1)[3], 255, "foreground stays opaque");
    }

    fn png_of(img: image::RgbaImage) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn recolor_masked_turns_yellow_green_and_keeps_shading() {
        // A "banana": bright yellow with a darker-yellow shaded pixel.
        let mut img = image::RgbaImage::from_pixel(4, 4, image::Rgba([230, 200, 40, 255]));
        img.put_pixel(1, 1, image::Rgba([150, 128, 20, 255])); // shaded
        let base = png_of(img);
        // Mask covers the left half only.
        let mut m = image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 0]));
        for y in 0..4 {
            for x in 0..2 {
                m.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let out = recolor_masked(&base, &png_of(m), "#22aa33").unwrap();
        let res = image::load_from_memory(&out).unwrap().to_rgba8();

        // Masked pixels became green (G dominates), unmasked stayed yellow.
        let p = res.get_pixel(0, 0);
        assert!(p[1] > p[0] && p[1] > p[2], "masked pixel is green: {:?}", p.0);
        assert_eq!(res.get_pixel(3, 3).0, [230, 200, 40, 255], "unmasked pixel untouched");

        // Shading survives: the shaded green pixel is darker than the lit one.
        let lit = res.get_pixel(0, 0);
        let shaded = res.get_pixel(1, 1);
        let lum = |p: &image::Rgba<u8>| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
        assert!(lum(shaded) < lum(lit), "shaded pixel stays darker after recolor");
        assert!(shaded[1] > shaded[0], "shaded pixel is also green: {:?}", shaded.0);
    }

    #[test]
    fn recolor_rejects_bad_color_and_empty_mask() {
        let base = sample_png();
        let empty_mask = png_of(image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 0, 0, 0])));
        let full_mask = png_of(image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 255, 255, 255])));
        assert!(recolor_masked(&base, &full_mask, "not-a-color").is_err());
        assert!(recolor_masked(&base, &empty_mask, "#22aa33").is_err());
    }

    #[test]
    fn neutralize_masked_hides_the_object_with_surrounding_color() {
        // Brown background with a yellow "banana" blob in the middle.
        let mut img = image::RgbaImage::from_pixel(16, 16, image::Rgba([92, 64, 40, 255]));
        for y in 6..10 {
            for x in 6..10 {
                img.put_pixel(x, y, image::Rgba([230, 200, 40, 255]));
            }
        }
        let base = png_of(img);
        // Mask exactly the blob.
        let mut m = image::RgbaImage::from_pixel(16, 16, image::Rgba([0, 0, 0, 0]));
        for y in 6..10 {
            for x in 6..10 {
                m.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let out = neutralize_masked(&base, &png_of(m)).unwrap();
        let res = image::load_from_memory(&out).unwrap().to_rgba8();
        // The blob is gone — filled with the surrounding brown, not yellow.
        let p = res.get_pixel(8, 8);
        assert!(
            (p[0] as i32 - 92).abs() < 12 && (p[1] as i32 - 64).abs() < 12,
            "masked region filled with surroundings: {:?}",
            p.0
        );
        // Unmasked pixels untouched.
        assert_eq!(res.get_pixel(0, 0).0, [92, 64, 40, 255]);
    }

    #[test]
    fn hsl_roundtrip_is_stable() {
        for (r, g, b) in [(230u8, 200u8, 40u8), (10, 200, 30), (128, 128, 128), (255, 0, 255)] {
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let (r2, g2, b2) = hsl_to_rgb(h, s, l);
            assert!((r as i32 - r2 as i32).abs() <= 2, "{r} vs {r2}");
            assert!((g as i32 - g2 as i32).abs() <= 2, "{g} vs {g2}");
            assert!((b as i32 - b2 as i32).abs() <= 2, "{b} vs {b2}");
        }
    }
}
