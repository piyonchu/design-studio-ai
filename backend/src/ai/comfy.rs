//! ComfyUI client — the local inference server (Pro pipeline §6, LOCAL_AI_SETUP).
//!
//! Active when `LOCAL_AI_URL` is set (e.g. `http://127.0.0.1:8188`); otherwise
//! the AI seams keep their mock / 503 behaviour. The API is "workflow JSON in →
//! images out": upload the input images, POST the graph to `/prompt`, poll
//! `/history/<id>` until it has outputs, then GET `/view`.
//!
//! The inpaint graph here is the Fooocus-inpaint workflow validated end-to-end
//! against this box (SDXL base + `comfyui-inpaint-nodes`): it changes only the
//! masked region and leaves the rest byte-stable.

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::AppError;

/// The ComfyUI base URL (trailing slash trimmed), or None when unset/empty.
pub fn local_url() -> Option<String> {
    std::env::var("LOCAL_AI_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
}

fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .expect("reqwest client builds")
    })
}

fn internal(ctx: &'static str) -> impl Fn(reqwest::Error) -> AppError {
    move |e| AppError::Internal(format!("comfyui {ctx}: {e}"))
}

/// Upload one PNG; returns the filename ComfyUI stored it as (LoadImage input).
async fn upload_image(base: &str, name: &str, bytes: &[u8]) -> Result<String, AppError> {
    let part = reqwest::multipart::Part::bytes(bytes.to_vec())
        .file_name(name.to_string())
        .mime_str("image/png")
        .map_err(internal("upload part"))?;
    let form = reqwest::multipart::Form::new()
        .part("image", part)
        .text("overwrite", "true");
    let v: Value = client()
        .post(format!("{base}/upload/image"))
        .multipart(form)
        .send()
        .await
        .map_err(internal("upload"))?
        .error_for_status()
        .map_err(internal("upload status"))?
        .json()
        .await
        .map_err(internal("upload json"))?;
    v["name"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| AppError::Internal("comfyui upload: response had no name".into()))
}

/// The Fooocus-inpaint workflow graph (ComfyUI API format), parameterised by the
/// uploaded base/mask filenames and the edit prompt. Mask: white = edit region.
fn inpaint_workflow(base_name: &str, mask_name: &str, prompt: &str, seed: u64) -> Value {
    json!({
        "ckpt": {"class_type": "CheckpointLoaderSimple",
                 "inputs": {"ckpt_name": "sd_xl_base_1.0.safetensors"}},
        "pos": {"class_type": "CLIPTextEncode", "inputs": {"text": prompt, "clip": ["ckpt", 1]}},
        "neg": {"class_type": "CLIPTextEncode",
                "inputs": {"text": "blurry, low quality, artifacts", "clip": ["ckpt", 1]}},
        "base": {"class_type": "LoadImage", "inputs": {"image": base_name}},
        "maskimg": {"class_type": "LoadImage", "inputs": {"image": mask_name}},
        "mask": {"class_type": "ImageToMask", "inputs": {"image": ["maskimg", 0], "channel": "red"}},
        "imc": {"class_type": "InpaintModelConditioning", "inputs": {
            "positive": ["pos", 0], "negative": ["neg", 0], "vae": ["ckpt", 2],
            "pixels": ["base", 0], "mask": ["mask", 0], "noise_mask": true}},
        "foo": {"class_type": "INPAINT_LoadFooocusInpaint", "inputs": {
            "head": "fooocus_inpaint_head.pth", "patch": "inpaint_v26.fooocus.patch"}},
        "apply": {"class_type": "INPAINT_ApplyFooocusInpaint", "inputs": {
            "model": ["ckpt", 0], "patch": ["foo", 0], "latent": ["imc", 2]}},
        "ks": {"class_type": "KSampler", "inputs": {
            "model": ["apply", 0], "seed": seed, "steps": 20, "cfg": 7.0,
            "sampler_name": "euler", "scheduler": "normal",
            "positive": ["imc", 0], "negative": ["imc", 1],
            "latent_image": ["imc", 2], "denoise": 1.0}},
        "dec": {"class_type": "VAEDecode", "inputs": {"samples": ["ks", 0], "vae": ["ckpt", 2]}},
        "save": {"class_type": "SaveImage",
                 "inputs": {"images": ["dec", 0], "filename_prefix": "canonforge_inpaint"}},
    })
}

async fn submit(base: &str, workflow: Value) -> Result<String, AppError> {
    let v: Value = client()
        .post(format!("{base}/prompt"))
        .json(&json!({"prompt": workflow, "client_id": "canonforge"}))
        .send()
        .await
        .map_err(internal("submit"))?
        .error_for_status()
        .map_err(internal("submit status"))?
        .json()
        .await
        .map_err(internal("submit json"))?;
    v["prompt_id"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| AppError::Internal("comfyui submit: no prompt_id".into()))
}

/// Poll `/history/<id>` until the job produces an output (or errors/times out).
/// Returns the output image's filename. ~2 min ceiling at 1s intervals.
async fn poll_output(base: &str, prompt_id: &str) -> Result<String, AppError> {
    for _ in 0..120 {
        let hist: Value = client()
            .get(format!("{base}/history/{prompt_id}"))
            .send()
            .await
            .map_err(internal("history"))?
            .json()
            .await
            .map_err(internal("history json"))?;
        let entry = &hist[prompt_id];
        if let Some(status) = entry.get("status").and_then(|s| s.get("status_str")).and_then(|s| s.as_str()) {
            if status == "error" {
                return Err(AppError::Internal(format!(
                    "comfyui job failed: {}",
                    entry.get("status").map(|s| s.to_string()).unwrap_or_default()
                )));
            }
        }
        if let Some(name) = entry
            .get("outputs")
            .and_then(|o| o.get("save"))
            .and_then(|s| s.get("images"))
            .and_then(|imgs| imgs.get(0))
            .and_then(|img| img.get("filename"))
            .and_then(|f| f.as_str())
        {
            return Ok(name.to_string());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(AppError::Internal("comfyui: job did not finish in time".into()))
}

async fn fetch_output(base: &str, filename: &str) -> Result<Vec<u8>, AppError> {
    let bytes = client()
        .get(format!("{base}/view"))
        .query(&[("filename", filename), ("type", "output"), ("subfolder", "")])
        .send()
        .await
        .map_err(internal("view"))?
        .error_for_status()
        .map_err(internal("view status"))?
        .bytes()
        .await
        .map_err(internal("view bytes"))?;
    Ok(bytes.to_vec())
}

/// Regenerate only the masked region of `base` per `prompt`. `mask` PNG: white
/// (red channel) = the region to change. Returns new full-image PNG bytes.
///
// ponytail: sends the whole image; crop-to-mask (§8 speed lever) can wrap this
// later — isolate the mask bbox, inpaint that, composite back.
pub async fn inpaint(base: &[u8], mask: &[u8], prompt: &str) -> Result<Vec<u8>, AppError> {
    let url = local_url()
        .ok_or_else(|| AppError::ServiceUnavailable("LOCAL_AI_URL not set".into()))?;
    let tag = Uuid::new_v4().simple().to_string();
    let base_name = upload_image(&url, &format!("cf_base_{tag}.png"), base).await?;
    let mask_name = upload_image(&url, &format!("cf_mask_{tag}.png"), mask).await?;
    let seed = u64::from_le_bytes(Uuid::new_v4().as_bytes()[..8].try_into().unwrap());
    let pid = submit(&url, inpaint_workflow(&base_name, &mask_name, prompt, seed)).await?;
    let filename = poll_output(&url, &pid).await?;
    fetch_output(&url, &filename).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn png(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut img = image::RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, image::Rgba(f(x, y)));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    /// Live end-to-end against a running ComfyUI. Skips (passes) when
    /// LOCAL_AI_URL is unset so CI/dev stays green:
    ///   LOCAL_AI_URL=http://127.0.0.1:8188 cargo test --lib \
    ///     comfy::tests::inpaint_live -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "needs a live ComfyUI (LOCAL_AI_URL)"]
    async fn inpaint_live() {
        if local_url().is_none() {
            eprintln!("skip: LOCAL_AI_URL unset");
            return;
        }
        let base = png(256, 256, |_, _| [120, 120, 120, 255]);
        // white square in the middle = the region to inpaint
        let mask = png(256, 256, |x, y| {
            if (85..171).contains(&x) && (85..171).contains(&y) {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 255]
            }
        });
        let out = inpaint(&base, &mask, "a glowing red gem").await.expect("inpaint ok");
        assert!(out.len() > 1000, "expected real image bytes, got {}", out.len());
        assert_eq!(&out[1..4], b"PNG", "expected a PNG");
    }
}
