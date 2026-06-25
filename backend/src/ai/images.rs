//! Image generation via OpenRouter (server-side; key never leaves the backend).
//!
//! Modes: `ASSET_MOCK=true` (default) → deterministic picsum placeholder URLs,
//! no cost; else `OPENROUTER_API_KEY` set → real generation
//! (`google/gemini-2.5-flash-image` by default), returning a base64 data URL;
//! else → 503. Real S3 storage is deferred — the URL/data-URL is stored as-is.

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{json, Value};

use crate::error::AppError;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_IMAGE_MODEL: &str = "google/gemini-2.5-flash-image";
const TIMEOUT_SECS: u64 = 90;
const MAX_RETRIES: u32 = 2;

fn asset_mock() -> bool {
    std::env::var("ASSET_MOCK").map(|v| v == "true").unwrap_or(false)
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

/// Generate one image for `prompt` (variant `n`), returning a URL or data URL.
pub async fn generate_image(prompt: &str, n: usize) -> Result<String, AppError> {
    if asset_mock() {
        return Ok(format!("https://picsum.photos/seed/{}/512/512", seed(prompt, n)));
    }
    let key = api_key().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "Image generation not configured: set OPENROUTER_API_KEY, or ASSET_MOCK=true".into(),
        )
    })?;

    let body = json!({
        "model": model(),
        "messages": [{ "role": "user", "content": prompt }],
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
                return extract_image_url(&payload);
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
