//! Text LLM behind the same boundary as images/audio, used to *synthesize* an
//! answer from retrieved project context ("Ask this project"). Mock-first; real
//! calls go through a cheap model AND a disk cache, so an identical question is
//! never paid for twice.
//!
//!   - `LLM_MOCK=true` (default) → a templated answer from the top snippet, no
//!     network/cost.
//!   - `OPENROUTER_API_KEY` set + `LLM_MOCK=false` → a real synthesis via a cheap
//!     text model (`OPENROUTER_LLM_MODEL`, default `google/gemini-2.0-flash-001`),
//!     cached under `AI_CACHE_DIR`.

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{json, Value};

use crate::ai::cache;
use crate::error::AppError;

const URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_MODEL: &str = "google/gemini-2.5-flash";
const TIMEOUT_SECS: u64 = 30;
const MAX_TOKENS: u32 = 350;

fn llm_mock() -> bool {
    std::env::var("LLM_MOCK").map(|v| v.trim().eq_ignore_ascii_case("true")).unwrap_or(true)
}
fn api_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.trim().is_empty())
}
fn model() -> String {
    std::env::var("OPENROUTER_LLM_MODEL").ok().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| DEFAULT_MODEL.to_string())
}
fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder().timeout(Duration::from_secs(TIMEOUT_SECS)).build().expect("client")
    })
}

fn build_prompt(question: &str, notes: &[String]) -> String {
    let context = notes
        .iter()
        .enumerate()
        .map(|(i, n)| format!("[{}] {}", i + 1, n))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You are a concise assistant for a visual-asset project. Answer the question \
         using ONLY the project notes below; if they don't cover it, say so. Keep it to \
         2-4 sentences.\n\nProject notes:\n{context}\n\nQuestion: {question}\n\nAnswer:"
    )
}

/// A short, honest stand-in so dev/CI work with no key/cost.
fn mock_answer(notes: &[String]) -> String {
    let top = notes.first().map(String::as_str).unwrap_or("(none)");
    format!(
        "Based on {} project note(s), the most relevant is: “{}”. \
         (Set LLM_MOCK=false + OPENROUTER_API_KEY for a synthesized answer.)",
        notes.len(),
        top.chars().take(160).collect::<String>()
    )
}

/// Synthesize an answer from retrieved notes. Cached by (model, prompt).
pub async fn synthesize(question: &str, notes: &[String]) -> Result<String, AppError> {
    if notes.is_empty() {
        return Ok("No relevant project context found for that question yet.".to_string());
    }
    if llm_mock() {
        return Ok(mock_answer(notes));
    }
    let key = api_key().ok_or_else(|| {
        AppError::ServiceUnavailable("LLM not configured: set OPENROUTER_API_KEY, or LLM_MOCK=true".into())
    })?;
    let m = model();
    let prompt = build_prompt(question, notes);
    let ck = cache::key(&[&m, &prompt]);
    if let Some(hit) = cache::get("llm", &ck) {
        return Ok(hit);
    }
    let answer = call(&key, &m, &prompt).await?;
    cache::put("llm", &ck, &answer);
    Ok(answer)
}

async fn call(key: &str, model: &str, prompt: &str) -> Result<String, AppError> {
    let body = json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "max_tokens": MAX_TOKENS,
        "temperature": 0.2,
    });
    let resp = client()
        .post(URL)
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("LLM request failed: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        return Err(AppError::ServiceUnavailable(format!("LLM provider returned {status}")));
    }
    let v: Value = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("LLM decode failed: {e}")))?;
    let answer = v["choices"][0]["message"]["content"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Internal("LLM returned no content".into()))?;
    Ok(answer.to_string())
}
