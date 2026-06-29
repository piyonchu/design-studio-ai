//! Masked / inpaint editing behind a provider seam (Pro pipeline B2).
//!
//! Modes mirror the other AI boundaries:
//!   - `EDIT_MOCK=true` (**default**) → a deterministic local mock that visibly
//!     alters ONLY the masked region (proves the mask plumbing end-to-end with
//!     no network and no cost).
//!   - `EDIT_MOCK=false` → a real inpaint provider (fal.ai / Replicate
//!     SD-inpaint) plugs in here. Deferred (new key + per-edit spend); until
//!     wired it returns 503 so the seam is honest.

use std::io::Cursor;

use image::{GenericImageView, ImageFormat};

use crate::ai::images::GeneratedImage;
use crate::error::AppError;

/// Default ON: dev/CI/demo inpaint is the free local mock unless explicitly
/// turned off to use a real provider.
pub fn is_mock() -> bool {
    std::env::var("EDIT_MOCK")
        .map(|v| !v.trim().eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}

/// Regenerate only the masked region of `base` according to `prompt`. `mask` is
/// an image whose painted (opaque + bright) pixels mark the region to change.
/// Returns new full-image PNG bytes.
pub async fn inpaint(base: &[u8], mask: &[u8], prompt: &str) -> Result<GeneratedImage, AppError> {
    if is_mock() {
        return mock_inpaint(base, mask, prompt);
    }
    // Real provider deferred — decision (fal.ai vs Replicate) made at wiring
    // time, plus a new key + per-edit spend. Keep the seam honest until then.
    Err(AppError::ServiceUnavailable(
        "inpaint provider not configured (set EDIT_MOCK=true for the local mock)".into(),
    ))
}

/// Deterministic mock: tint + darken the masked region toward a prompt-derived
/// hue, leaving everything outside the mask byte-identical. Enough to prove the
/// "change only this region → new version" flow and demo the diff slider.
fn mock_inpaint(base: &[u8], mask: &[u8], prompt: &str) -> Result<GeneratedImage, AppError> {
    let base_img = image::load_from_memory(base)
        .map_err(|e| AppError::BadRequest(format!("cannot decode base image: {e}")))?;
    let (w, h) = base_img.dimensions();
    let mut out = base_img.to_rgba8();

    let mask_img = image::load_from_memory(mask)
        .map_err(|e| AppError::BadRequest(format!("cannot decode mask image: {e}")))?;
    // Align the mask to the base (the brush canvas should match, but be lenient).
    let mask_rgba = if mask_img.dimensions() == (w, h) {
        mask_img.to_rgba8()
    } else {
        image::imageops::resize(&mask_img.to_rgba8(), w, h, image::imageops::FilterType::Nearest)
    };

    let (tr, tg, tb) = prompt_tint(prompt);
    let mut touched = 0u64;
    for y in 0..h {
        for x in 0..w {
            let m = mask_rgba.get_pixel(x, y);
            let lum = 0.299 * m[0] as f32 + 0.587 * m[1] as f32 + 0.114 * m[2] as f32;
            // Painted region: opaque-ish AND bright (works for white-on-clear
            // and white-on-black masks alike).
            if m[3] > 32 && lum > 96.0 {
                let p = out.get_pixel_mut(x, y);
                // 55% toward the tint, then a touch darker — a clear, localized change.
                p[0] = blend(p[0], tr, 0.55);
                p[1] = blend(p[1], tg, 0.55);
                p[2] = blend(p[2], tb, 0.55);
                for c in 0..3 {
                    p[c] = (p[c] as f32 * 0.85) as u8;
                }
                touched += 1;
            }
        }
    }
    if touched == 0 {
        return Err(AppError::BadRequest("mask is empty — paint the region to edit".into()));
    }

    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("encode failed: {e}")))?;
    Ok(GeneratedImage { bytes: buf.into_inner(), mime: "image/png".into() })
}

fn blend(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 * (1.0 - t) + b as f32 * t) as u8
}

/// A stable tint colour derived from the prompt, so different edits read
/// differently in the mock (FNV-1a → RGB).
fn prompt_tint(prompt: &str) -> (u8, u8, u8) {
    let mut hsh = 1469598103934665603u64;
    for b in prompt.bytes() {
        hsh ^= b as u64;
        hsh = hsh.wrapping_mul(1099511628211);
    }
    // Spread the bits across channels and bias bright so the change is visible.
    let r = 80 + (hsh & 0x7f) as u8;
    let g = 80 + ((hsh >> 8) & 0x7f) as u8;
    let b = 80 + ((hsh >> 16) & 0x7f) as u8;
    (r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(color: [u8; 4], w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba(color));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[tokio::test]
    async fn mock_changes_only_the_masked_region() {
        std::env::set_var("EDIT_MOCK", "true");
        let base = png([200, 200, 200, 255], 4, 4);

        // Mask: paint only the top-left pixel white-opaque; rest transparent.
        let mut mask = image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 0]));
        mask.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
        let mut mbuf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(mask).write_to(&mut mbuf, ImageFormat::Png).unwrap();

        let out = inpaint(&base, &mbuf.into_inner(), "red hat").await.unwrap();
        let res = image::load_from_memory(&out.bytes).unwrap().to_rgba8();
        assert_ne!(res.get_pixel(0, 0).0, [200, 200, 200, 255], "masked pixel changed");
        assert_eq!(res.get_pixel(1, 1).0, [200, 200, 200, 255], "unmasked pixel unchanged");
    }

    #[tokio::test]
    async fn empty_mask_is_rejected() {
        std::env::set_var("EDIT_MOCK", "true");
        let base = png([10, 10, 10, 255], 3, 3);
        let mask = png([0, 0, 0, 0], 3, 3); // nothing painted
        assert!(inpaint(&base, &mask, "x").await.is_err());
    }
}
