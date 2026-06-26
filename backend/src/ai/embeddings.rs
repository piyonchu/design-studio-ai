//! Embedding boundary for RAG / smart search / dedup, behind `EMBED_MOCK`.
//!
//! - `EMBED_MOCK=true` (default) → a **feature-hashed bag-of-words** embedder:
//!   deterministic, free, lexical. Good enough to test the whole pipeline.
//! - `EMBED_MOCK=false` + `OPENROUTER_API_KEY` → a **real text embedder**
//!   (`openai/text-embedding-3-small` via OpenRouter `/embeddings`, with the
//!   `dimensions` param matching our columns). True semantic similarity. Each
//!   (model, dim, text) is cached on disk (`ai::cache`), so a given text is
//!   embedded — and paid for — at most once. Visual embeddings use the asset's
//!   caption (real pixel-CLIP is a future swap behind this same seam).

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::ai::cache;
use crate::error::AppError;
use crate::models::Asset;

/// Vector dimensions match the schema (`visual_embeddings` = 768,
/// `semantic_embeddings` = 1024).
pub const VISUAL_DIM: usize = 768;
pub const SEMANTIC_DIM: usize = 1024;

const EMBED_URL: &str = "https://openrouter.ai/api/v1/embeddings";
const DEFAULT_EMBED_MODEL: &str = "openai/text-embedding-3-small";

fn embed_mock() -> bool {
    std::env::var("EMBED_MOCK").map(|v| v.trim().eq_ignore_ascii_case("true")).unwrap_or(true)
}
fn api_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.trim().is_empty())
}
fn real_model() -> String {
    std::env::var("EMBED_MODEL").ok().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| DEFAULT_EMBED_MODEL.to_string())
}
fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(30)).build().expect("client"))
}

/// The model tag stored on each embedding row (so a future re-embed can tell
/// mock rows from real ones).
pub fn model_tag() -> String {
    if embed_mock() {
        "mock-bow-v1".to_string()
    } else {
        real_model()
    }
}

fn fnv(s: &str) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// The deterministic, free feature-hashed embedder (mock + the unit-test core).
pub fn embed_mock_vec(text: &str, dim: usize) -> Option<Vec<f32>> {
    let mut v = vec![0f32; dim];
    let mut any = false;
    for tok in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|t| t.len() > 1) {
        let h = fnv(tok);
        let idx = (h % dim as u64) as usize;
        let sign = if (h >> 40) & 1 == 0 { 1.0 } else { -1.0 };
        v[idx] += sign;
        any = true;
    }
    if !any {
        return None;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    Some(v)
}

/// Real text embedding via OpenRouter, cached by (model, dim, text). Returns
/// None on any failure so indexing degrades gracefully (search just returns less).
async fn embed_real(text: &str, dim: usize) -> Option<Vec<f32>> {
    let key = api_key()?;
    let model = real_model();
    let ck = cache::key(&[&model, &dim.to_string(), text]);
    if let Some(hit) = cache::get("embed", &ck) {
        return parse_csv(&hit);
    }
    let body = json!({ "model": model, "input": text, "dimensions": dim });
    let resp = client().post(EMBED_URL).bearer_auth(&key).json(&body).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::warn!(status = %resp.status(), "embedding request failed");
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    let arr = v["data"][0]["embedding"].as_array()?;
    let vec: Vec<f32> = arr.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
    if vec.len() != dim {
        tracing::warn!(got = vec.len(), want = dim, "embedding dim mismatch");
        return None;
    }
    cache::put("embed", &ck, &vec.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(","));
    Some(vec)
}

fn parse_csv(s: &str) -> Option<Vec<f32>> {
    let v: Vec<f32> = s.split(',').filter_map(|p| p.parse().ok()).collect();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

/// Embed text → an L2-normalized (mock) or provider (real) vector, or `None`
/// when there's nothing to embed / no provider. Async so the real path can do IO.
pub async fn embed_text(text: &str, dim: usize) -> Option<Vec<f32>> {
    if text.trim().is_empty() {
        return None;
    }
    if embed_mock() {
        embed_mock_vec(text, dim)
    } else {
        embed_real(text, dim).await
    }
}

/// pgvector text form: `[a,b,c]`. Bound as text and cast `$n::vector` in SQL so
/// we need no extra crate.
pub fn to_pgvector(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 8);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{x:.6}"));
    }
    s.push(']');
    s
}

/// The text we embed for an asset — its searchable identity. Works for imports
/// (role/tags) just as well as generated assets (prompt/derivation).
pub fn caption_from(a: &Asset) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(r) = a.role.as_deref() {
        parts.push(r);
    }
    if let Some(p) = a.prompt.as_deref() {
        parts.push(p);
    }
    if let Some(d) = a.derivation.as_deref() {
        parts.push(d);
    }
    for t in &a.tags {
        parts.push(t);
    }
    parts.join(" ")
}

/// Index (or re-index) one asset's embedding. No-op when the caption is empty or
/// no embedder is configured — never fails the caller's main operation.
pub async fn index_asset(pool: &PgPool, asset_id: Uuid, caption: &str) -> Result<(), AppError> {
    let Some(v) = embed_text(caption, VISUAL_DIM).await else {
        return Ok(());
    };
    let pg = to_pgvector(&v);
    sqlx::query("DELETE FROM visual_embeddings WHERE asset_id = $1")
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO visual_embeddings (asset_id, embedding, model) VALUES ($1, $2::vector, $3)")
        .bind(asset_id)
        .bind(pg)
        .bind(model_tag())
        .execute(pool)
        .await?;
    Ok(())
}

/// Best-effort index of an asset across BOTH stores: the visual embedding (for
/// search/dedup) and a semantic-context row for its prompt/derivation (so
/// "why does this asset exist?" retrieval can find it). Never fails the caller.
pub async fn index_asset_soft(pool: &PgPool, a: &Asset) {
    if let Err(e) = index_asset(pool, a.id, &caption_from(a)).await {
        tracing::warn!(error = %e, asset = %a.id, "asset visual embedding failed (non-fatal)");
    }
    let rationale = a.prompt.as_deref().or(a.derivation.as_deref());
    if let Some(text) = rationale {
        if let Err(e) = index_semantic(pool, a.project_id, "asset_prompt", Some(a.id), text).await {
            tracing::warn!(error = %e, asset = %a.id, "asset semantic embedding failed (non-fatal)");
        }
    }
}

/// Index a text snippet into the semantic-context store. Re-indexing the same
/// (source_kind, source_id) replaces the old row. No-op when empty / no embedder.
pub async fn index_semantic(
    pool: &PgPool,
    project_id: Uuid,
    source_kind: &str,
    source_id: Option<Uuid>,
    content: &str,
) -> Result<(), AppError> {
    let Some(v) = embed_text(content, SEMANTIC_DIM).await else {
        return Ok(());
    };
    let pg = to_pgvector(&v);
    if let Some(sid) = source_id {
        sqlx::query("DELETE FROM semantic_embeddings WHERE source_kind = $1 AND source_id = $2")
            .bind(source_kind)
            .bind(sid)
            .execute(pool)
            .await?;
    }
    sqlx::query(
        "INSERT INTO semantic_embeddings (project_id, source_kind, source_id, content, embedding, model)
         VALUES ($1, $2, $3, $4, $5::vector, $6)",
    )
    .bind(project_id)
    .bind(source_kind)
    .bind(source_id)
    .bind(content)
    .bind(pg)
    .bind(model_tag())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{embed_mock_vec as embed_text, to_pgvector, VISUAL_DIM};

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum() // vectors are L2-normalized
    }

    #[test]
    fn identical_text_is_self_similar() {
        let a = embed_text("cute japanese hat", VISUAL_DIM).unwrap();
        let b = embed_text("cute japanese hat", VISUAL_DIM).unwrap();
        assert!((cosine(&a, &b) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn overlap_beats_disjoint() {
        let q = embed_text("japanese hat", VISUAL_DIM).unwrap();
        let near = embed_text("a cute japanese straw hat", VISUAL_DIM).unwrap();
        let far = embed_text("metal sword clang", VISUAL_DIM).unwrap();
        assert!(cosine(&q, &near) > cosine(&q, &far));
    }

    #[test]
    fn empty_or_tokenless_is_none() {
        assert!(embed_text("", VISUAL_DIM).is_none());
        assert!(embed_text("  ! ? ", VISUAL_DIM).is_none());
    }

    #[test]
    fn vectors_are_normalized() {
        let v = embed_text("hello world foo bar", VISUAL_DIM).unwrap();
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[test]
    fn pgvector_text_format() {
        assert_eq!(to_pgvector(&[1.0, 2.5]), "[1.000000,2.500000]");
    }
}

/// Fire-and-forget semantic index (logs on failure).
pub async fn index_semantic_soft(
    pool: &PgPool,
    project_id: Uuid,
    source_kind: &str,
    source_id: Option<Uuid>,
    content: &str,
) {
    if let Err(e) = index_semantic(pool, project_id, source_kind, source_id, content).await {
        tracing::warn!(error = %e, kind = source_kind, "semantic embedding failed (non-fatal)");
    }
}
