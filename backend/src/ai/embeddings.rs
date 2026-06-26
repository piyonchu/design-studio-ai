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

/// Vector dimensions match the schema (`visual_embeddings` = 768,
/// `semantic_embeddings` = 1024).
pub const VISUAL_DIM: usize = 768;
pub const SEMANTIC_DIM: usize = 1024;
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
    let Some(v) = embed_text(content, SEMANTIC_DIM) else {
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
    .bind(MODEL)
    .execute(pool)
    .await?;
    Ok(())
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
