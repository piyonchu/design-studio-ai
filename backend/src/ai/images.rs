//! Image generation via OpenRouter (server-side; key never leaves the backend).
//!
//! Returns raw image bytes + MIME so the caller can persist them to object
//! storage (see `crate::storage`). Three modes:
//!   - `ASSET_MOCK=true` (default) → a deterministic local SVG placeholder,
//!     no network and no cost.
//!   - `OPENROUTER_API_KEY` set → real generation
//!     (`google/gemini-2.5-flash-image` by default); the model returns a base64
//!     `data:` URL which we decode (or an http URL which we fetch).
//!   - neither → 503.

use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine;
use serde_json::{json, Value};

use crate::error::AppError;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_IMAGE_MODEL: &str = "google/gemini-2.5-flash-image";
const TIMEOUT_SECS: u64 = 90;
const MAX_RETRIES: u32 = 2;

/// A generated image ready to persist.
pub struct GeneratedImage {
    pub bytes: Vec<u8>,
    pub mime: String,
}

fn asset_mock() -> bool {
    std::env::var("ASSET_MOCK").map(|v| v.trim().eq_ignore_ascii_case("true")).unwrap_or(false)
}

fn api_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.trim().is_empty())
}

fn model() -> String {
    std::env::var("OPENROUTER_IMAGE_MODEL")
        .ok()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_IMAGE_MODEL.to_string())
}

fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("reqwest client builds")
    })
}

fn seed(prompt: &str, n: usize) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in prompt.bytes().chain([b'#', n as u8]) {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// Generate one image for `prompt` (variant `n`) from text alone.
pub async fn generate_image(prompt: &str, n: usize) -> Result<GeneratedImage, AppError> {
    if asset_mock() {
        return Ok(mock_image(prompt, n));
    }
    call_openrouter(&require_key()?, json!(prompt)).await
}

/// Derive one image conditioned on a `base` image (img2img): the base is sent
/// as a reference alongside the instruction, so identity + style carry over.
/// This is the load-bearing mechanism for consistent derivatives (see the spike).
pub async fn derive_image(
    base: &[u8],
    base_mime: &str,
    prompt: &str,
    n: usize,
) -> Result<GeneratedImage, AppError> {
    if asset_mock() {
        return Ok(mock_image(prompt, n));
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(base);
    let content = json!([
        { "type": "text", "text": prompt },
        { "type": "image_url", "image_url": { "url": format!("data:{base_mime};base64,{b64}") } },
    ]);
    call_openrouter(&require_key()?, content).await
}

fn mock_image(prompt: &str, n: usize) -> GeneratedImage {
    GeneratedImage {
        bytes: mock_svg(prompt, n).into_bytes(),
        mime: "image/svg+xml".to_string(),
    }
}

fn require_key() -> Result<String, AppError> {
    api_key().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "Image generation not configured: set OPENROUTER_API_KEY, or ASSET_MOCK=true".into(),
        )
    })
}

/// POST one chat-completions image request. `content` is a string (text→image)
/// or an array carrying a reference image (img2img). Retries transient failures,
/// then resolves the returned reference into bytes.
async fn call_openrouter(key: &str, content: Value) -> Result<GeneratedImage, AppError> {
    let body = json!({
        "model": model(),
        "messages": [{ "role": "user", "content": content }],
        "modalities": ["image", "text"],
    });

    let mut last = String::new();
    for attempt in 0..=MAX_RETRIES {
        let resp = client()
            .post(OPENROUTER_URL)
            .header("authorization", format!("Bearer {key}"))
            .header("content-type", "application/json")
            .header("x-title", "Design Studio AI")
            .json(&body)
            .send()
            .await;
        match resp {
            Ok(r) => {
                let status = r.status();
                if status.as_u16() == 429 || status.is_server_error() {
                    last = format!("upstream status {status}");
                    backoff(attempt).await;
                    continue;
                }
                let payload: Value = r
                    .json()
                    .await
                    .map_err(|e| AppError::ServiceUnavailable(format!("image bad response: {e}")))?;
                if let Some(err) = payload.get("error") {
                    return Err(AppError::ServiceUnavailable(format!(
                        "image generation failed: {}",
                        err.get("message").and_then(Value::as_str).unwrap_or("unknown")
                    )));
                }
                let url = extract_image_url(&payload)?;
                return resolve(&url).await;
            }
            Err(e) => {
                last = e.to_string();
                backoff(attempt).await;
            }
        }
    }
    Err(AppError::ServiceUnavailable(format!(
        "image service unreachable after retries: {last}"
    )))
}

async fn backoff(attempt: u32) {
    tokio::time::sleep(Duration::from_millis(400 * 2u64.pow(attempt))).await;
}

/// Turn the model's image reference into bytes: decode a `data:` URL inline,
/// or fetch an `http(s)` URL.
async fn resolve(url: &str) -> Result<GeneratedImage, AppError> {
    if let Some(rest) = url.strip_prefix("data:") {
        // data:<mime>;base64,<payload>
        let (meta, payload) = rest
            .split_once(',')
            .ok_or_else(|| AppError::ServiceUnavailable("malformed data URL".into()))?;
        let mime = meta.split(';').next().filter(|m| !m.is_empty()).unwrap_or("image/png");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload.trim())
            .map_err(|e| AppError::ServiceUnavailable(format!("image decode failed: {e}")))?;
        return Ok(GeneratedImage { bytes, mime: mime.to_string() });
    }
    // Otherwise assume an http(s) URL and fetch it.
    let r = client()
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("image fetch failed: {e}")))?;
    let mime = r
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .to_string();
    let bytes = r
        .bytes()
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("image fetch failed: {e}")))?
        .to_vec();
    Ok(GeneratedImage { bytes, mime })
}

/// Pull the generated image URL from `choices[0].message.images[0].image_url.url`.
fn extract_image_url(payload: &Value) -> Result<String, AppError> {
    payload
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("images"))
        .and_then(Value::as_array)
        .and_then(|imgs| imgs.first())
        .and_then(|im| im.get("image_url"))
        .and_then(|iu| iu.get("url"))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::ServiceUnavailable("image response had no image".into()))
}

/// A deterministic on-brand SVG placeholder (no network, no cost). Two seeded
/// brand hues, the prompt as a caption, and a subtle "AI placeholder" tag.
fn mock_svg(prompt: &str, n: usize) -> String {
    let s = seed(prompt, n);
    // Brand-ish palette: indigo / teal, nudged by the seed.
    let h1 = 190 + (s % 50) as u32; // teal-ish
    let h2 = 250 + ((s >> 8) % 40) as u32; // indigo-ish
    let label = escape_xml(&truncate(prompt, 48));
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512" role="img" aria-label="{label}">
  <defs>
    <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="hsl({h1} 70% 22%)"/>
      <stop offset="1" stop-color="hsl({h2} 65% 16%)"/>
    </linearGradient>
  </defs>
  <rect width="512" height="512" fill="url(#g)"/>
  <circle cx="256" cy="216" r="92" fill="none" stroke="hsl({h1} 80% 62%)" stroke-width="2" opacity="0.55"/>
  <path d="M256 168 l14 38 38 14 -38 14 -14 38 -14 -38 -38 -14 38 -14 z" fill="hsl({h1} 85% 66%)" opacity="0.9"/>
  <text x="256" y="356" fill="#e7e9ee" font-family="system-ui, sans-serif" font-size="22" font-weight="600" text-anchor="middle">{label}</text>
  <text x="256" y="392" fill="#9aa0ad" font-family="system-ui, sans-serif" font-size="14" text-anchor="middle">AI placeholder &#183; ASSET_MOCK</text>
</svg>"##
    )
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let mut out: String = t.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
