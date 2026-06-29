//! Deterministic, model-free image edits (Pro pipeline B1). Each op transforms
//! raw bytes → raw bytes using the `image` crate; the route records the result
//! as a new version (A2), so edits are **free, instant, and non-destructive** —
//! the original is always a prior version you can roll back to.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat};
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
}
