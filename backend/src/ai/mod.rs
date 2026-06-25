//! Anthropic integration: generate / patch the UI-as-Code DSL.
//!
//! Raw HTTP against the Messages API (Rust has no official SDK). All access
//! goes through this module so the transport can be swapped later. Three modes:
//!   - `AI_MOCK=true`       → canned, schema-valid DSL, no network (free dev)
//!   - `ANTHROPIC_API_KEY`  → real Messages API call
//!   - neither              → 503 "AI not configured"

pub mod dsl;
pub mod images;

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{json, Value};

use crate::error::AppError;
use crate::models::ArtifactKind;

const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-opus-4-8";
const MAX_TOKENS: u32 = 16000;
const TIMEOUT_SECS: u64 = 60;
const MAX_RETRIES: u32 = 2;

fn mock_enabled() -> bool {
    std::env::var("AI_MOCK").map(|v| v == "true").unwrap_or(false)
}

fn api_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
}

fn model() -> String {
    std::env::var("ANTHROPIC_MODEL")
        .ok()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

fn effort() -> String {
    std::env::var("ANTHROPIC_EFFORT")
        .ok()
        .filter(|e| !e.trim().is_empty())
        .unwrap_or_else(|| "medium".to_string())
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("reqwest client builds")
    })
}

/// Generate (or regenerate) a DSL tree for `kind`. `context` is the current /
/// parent DSL the edit should build on, if any. Validates the model's JSON and
/// retries once with the error fed back; fails gracefully as 503.
pub async fn generate_dsl(
    kind: ArtifactKind,
    prompt: &str,
    context: Option<&Value>,
) -> Result<Value, AppError> {
    if mock_enabled() {
        return Ok(dsl::mock_dsl(kind, prompt));
    }
    let key = api_key().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "AI not configured: set ANTHROPIC_API_KEY, or AI_MOCK=true for local dev".into(),
        )
    })?;

    let system = system_prompt(kind);
    let mut user = user_prompt(kind, prompt, context);

    for attempt in 0..=1 {
        let raw = call_messages(&key, &system, &user).await?;
        match extract_json(&raw).and_then(|v| dsl::validate(kind, &v).map(|_| v)) {
            Ok(v) => return Ok(v),
            Err(err) if attempt == 0 => {
                tracing::warn!(%err, "AI returned invalid DSL; retrying once");
                user = format!(
                    "{user}\n\nYour previous response was invalid: {err}\nReturn ONLY corrected JSON."
                );
            }
            Err(err) => {
                return Err(AppError::ServiceUnavailable(format!(
                    "AI returned invalid DSL after retry: {err}"
                )));
            }
        }
    }
    unreachable!()
}

fn system_prompt(kind: ArtifactKind) -> String {
    format!(
        "You are a product-design assistant for a UI-as-Code studio. You output \
         structural design artifacts as JSON only — never prose, never markdown, \
         never code fences. The artifact kind is `{kind:?}`. Return a single JSON \
         object exactly matching this shape:\n{}",
        dsl::dsl_spec(kind)
    )
}

fn user_prompt(kind: ArtifactKind, prompt: &str, context: Option<&Value>) -> String {
    match context {
        Some(ctx) => format!(
            "Current `{kind:?}` JSON:\n{}\n\nApply this change and return the FULL updated JSON:\n{prompt}",
            serde_json::to_string_pretty(ctx).unwrap_or_default()
        ),
        None => format!("Create a `{kind:?}` for this request:\n{prompt}"),
    }
}

/// POST to the Messages API; retry transient failures; return the first text block.
async fn call_messages(key: &str, system: &str, user: &str) -> Result<String, AppError> {
    let body = json!({
        "model": model(),
        "max_tokens": MAX_TOKENS,
        "thinking": { "type": "adaptive" },
        "output_config": { "effort": effort() },
        "system": system,
        "messages": [{ "role": "user", "content": user }],
    });

    let mut last_err = String::new();
    for attempt in 0..=MAX_RETRIES {
        let resp = client()
            .post(MESSAGES_URL)
            .header("x-api-key", key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status();
                // Retry rate-limit / server errors with backoff.
                if status.as_u16() == 429 || status.is_server_error() {
                    last_err = format!("upstream status {status}");
                    backoff(attempt).await;
                    continue;
                }
                if !status.is_success() {
                    let text = r.text().await.unwrap_or_default();
                    return Err(AppError::ServiceUnavailable(format!(
                        "AI request failed ({status}): {}",
                        text.chars().take(200).collect::<String>()
                    )));
                }
                let payload: Value = r
                    .json()
                    .await
                    .map_err(|e| AppError::ServiceUnavailable(format!("AI bad response: {e}")))?;
                return first_text_block(&payload);
            }
            Err(e) => {
                last_err = e.to_string();
                backoff(attempt).await;
            }
        }
    }
    Err(AppError::ServiceUnavailable(format!(
        "AI unreachable after retries: {last_err}"
    )))
}

async fn backoff(attempt: u32) {
    // 250ms, 500ms, 1s …
    let ms = 250u64 * 2u64.pow(attempt);
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

fn first_text_block(payload: &Value) -> Result<String, AppError> {
    payload
        .get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .and_then(|b| b.get("text"))
                .and_then(Value::as_str)
        })
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::ServiceUnavailable("AI response had no text block".into()))
}

/// Strip optional markdown code fences and isolate the JSON object, then parse.
fn extract_json(text: &str) -> Result<Value, String> {
    let mut s = text.trim();
    if let Some(rest) = s.strip_prefix("```") {
        // drop the opening fence (and an optional language tag on the same line)
        let rest = rest.splitn(2, '\n').nth(1).unwrap_or(rest);
        s = rest.trim_end().strip_suffix("```").unwrap_or(rest).trim();
    }
    // Fallback: clip to the outermost braces if there's surrounding noise.
    let candidate = match (s.find('{'), s.rfind('}')) {
        (Some(a), Some(b)) if b > a => &s[a..=b],
        _ => s,
    };
    serde_json::from_str(candidate).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_plain_json() {
        let v = extract_json(r#"{"text":"hi"}"#).unwrap();
        assert_eq!(v["text"], "hi");
    }

    #[test]
    fn strips_code_fences() {
        let v = extract_json("```json\n{\"text\":\"hi\"}\n```").unwrap();
        assert_eq!(v["text"], "hi");
    }

    #[test]
    fn clips_surrounding_prose() {
        let v = extract_json("Here you go:\n{\"text\":\"hi\"}\nHope that helps").unwrap();
        assert_eq!(v["text"], "hi");
    }
}
