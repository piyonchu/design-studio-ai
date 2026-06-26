//! Embedding boundary for RAG / smart search / dedup. Mock-first, exactly like
//! the image + audio clients, so a real model swaps in without touching routes.
//!
//! `EMBED_MOCK=true` (default) uses a **feature-hashed bag-of-words** embedder:
//! tokens hash into dims, L2-normalized. It's deterministic and genuinely
//! useful — identical text → cosine 1.0 (dedup), shared tokens → positive
//! similarity (keyword/overlap search) — so the whole pipeline is testable with
//! no spend. A real text/CLIP model (true semantic "feel") is a localized swap.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::Asset;

/// Vector dimensions match the schema (`visual_embeddings` = 768).
pub const VISUAL_DIM: usize = 768;
pub const MODEL: &str = "mock-bow-v1";

fn embed_mock() -> bool {
    std::env::var("EMBED_MOCK").map(|v| v.trim().eq_ignore_ascii_case("true")).unwrap_or(true)
}

fn fnv(s: &str) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// Embed text → an L2-normalized vector, or `None` if there are no tokens (an
/// empty caption carries no signal, so we don't index it). Real providers will
/// return their own vector here behind the same signature.
pub fn embed_text(text: &str, dim: usize) -> Option<Vec<f32>> {
    if !embed_mock() {
        // No hosted embedding provider wired yet — callers treat None as
        // "skip indexing" so the app still works; search just returns less.
        return None;
    }
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
    let Some(v) = embed_text(caption, VISUAL_DIM) else {
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
        .bind(MODEL)
        .execute(pool)
        .await?;
    Ok(())
}

/// Best-effort index: log and swallow errors so generation/upload never 500s on
/// an embedding hiccup.
pub async fn index_asset_soft(pool: &PgPool, a: &Asset) {
    if let Err(e) = index_asset(pool, a.id, &caption_from(a)).await {
        tracing::warn!(error = %e, asset = %a.id, "asset embedding failed (non-fatal)");
    }
}
