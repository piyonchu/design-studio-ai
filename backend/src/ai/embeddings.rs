//! Embedding boundary for smart search / dedup / RAG.
//!
//! Each image asset stores **two** vectors in `visual_embeddings`:
//! - `embedding_text` — caption metadata (role, prompt, derivation, tags)
//! - `embedding_visual` — pixel/multimodal embedding from the image bytes
//!
//! Mock mode (`EMBED_MOCK=true`, default): free deterministic embedders.
//! Real mode (`EMBED_MOCK=false` + `OPENROUTER_API_KEY`):
//! - text → `openai/text-embedding-3-small` (or `EMBED_TEXT_MODEL`)
//! - visual → `google/gemini-embedding-2-preview` (or `EMBED_VISUAL_MODEL`)
//!
//! All provider calls are disk-cached under `AI_CACHE_DIR`. Mirrored assets also
//! get `<asset_id>.embed.json` sidecars when `ASSET_MIRROR_DIR` is set.

use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine;
use image::GenericImageView;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::ai::cache;
use crate::error::AppError;
use crate::mirror;
use crate::models::Asset;

/// Vector dimensions match the schema (`visual_embeddings` = 768,
/// `semantic_embeddings` = 1024).
pub const VISUAL_DIM: usize = 768;
pub const SEMANTIC_DIM: usize = 1024;

const EMBED_URL: &str = "https://openrouter.ai/api/v1/embeddings";
const DEFAULT_TEXT_MODEL: &str = "openai/text-embedding-3-small";
const DEFAULT_VISUAL_MODEL: &str = "google/gemini-embedding-2-preview";

fn embed_mock() -> bool {
    std::env::var("EMBED_MOCK").map(|v| v.trim().eq_ignore_ascii_case("true")).unwrap_or(true)
}

fn api_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.trim().is_empty())
}

fn text_model() -> String {
    std::env::var("EMBED_TEXT_MODEL")
        .or_else(|_| std::env::var("EMBED_MODEL"))
        .ok()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_TEXT_MODEL.to_string())
}

fn visual_model() -> String {
    std::env::var("EMBED_VISUAL_MODEL")
        .ok()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_VISUAL_MODEL.to_string())
}

fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(60)).build().expect("client"))
}

pub fn model_tag_text() -> String {
    if embed_mock() { "mock-bow-v1".to_string() } else { text_model() }
}

pub fn model_tag_visual() -> String {
    if embed_mock() { "mock-pixel-v1".to_string() } else { visual_model() }
}

/// Legacy alias used by semantic-context rows.
pub fn model_tag() -> String {
    model_tag_text()
}

fn fnv(s: &str) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

fn fnv_bytes(bytes: &[u8]) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

pub fn bytes_sha(bytes: &[u8]) -> String {
    format!("{:016x}-{}", fnv_bytes(bytes), bytes.len())
}

/// The deterministic, free feature-hashed text embedder (mock).
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
    normalize(&mut v);
    Some(v)
}

/// Deterministic pixel embedder for mock mode — decodes raster images when
/// possible, otherwise hashes raw bytes.
pub fn embed_image_mock(bytes: &[u8], dim: usize) -> Option<Vec<f32>> {
    if bytes.is_empty() {
        return None;
    }
    let mut v = vec![0f32; dim];

    if let Ok(img) = image::load_from_memory(bytes) {
        let (w, h) = img.dimensions();
        let step_x = (w / 16).max(1);
        let step_y = (h / 16).max(1);
        for y in (0..h).step_by(step_y as usize) {
            for x in (0..w).step_by(step_x as usize) {
                let px = img.get_pixel(x, y).0;
                for (i, &c) in px.iter().enumerate() {
                    let h = fnv_bytes(&[c, (x % 256) as u8, (y % 256) as u8, i as u8]);
                    let idx = (h % dim as u64) as usize;
                    let sign = if (h >> 40) & 1 == 0 { 1.0 } else { -1.0 };
                    v[idx] += sign * (c as f32 / 255.0);
                }
            }
        }
    } else {
        for chunk in bytes.chunks(64) {
            let h = fnv_bytes(chunk);
            let idx = (h % dim as u64) as usize;
            let sign = if (h >> 40) & 1 == 0 { 1.0 } else { -1.0 };
            v[idx] += sign;
        }
    }

    normalize(&mut v);
    Some(v)
}

fn normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v {
            *x /= norm;
        }
    }
}

fn parse_csv(s: &str) -> Option<Vec<f32>> {
    let v: Vec<f32> = s.split(',').filter_map(|p| p.parse().ok()).collect();
    if v.is_empty() { None } else { Some(v) }
}

fn vec_csv(v: &[f32]) -> String {
    v.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",")
}

async fn embed_openrouter_text(model: &str, text: &str, dim: usize, namespace: &str) -> Option<Vec<f32>> {
    let key = api_key()?;
    let ck = cache::key(&[model, &dim.to_string(), text]);
    if let Some(hit) = cache::get(namespace, &ck) {
        return parse_csv(&hit);
    }
    let body = json!({ "model": model, "input": text, "dimensions": dim });
    let vec = post_embedding(&key, body).await?;
    if vec.len() != dim {
        tracing::warn!(got = vec.len(), want = dim, "text embedding dim mismatch");
        return None;
    }
    cache::put(namespace, &ck, &vec_csv(&vec));
    Some(vec)
}

/// Multimodal embedding (text and/or image) via OpenRouter — used for visual
/// pixels and cross-modal query vectors in the visual embedding space.
async fn embed_openrouter_multimodal(
    model: &str,
    content: Value,
    dim: usize,
    cache_parts: &[&str],
    namespace: &str,
) -> Option<Vec<f32>> {
    let key = api_key()?;
    let ck = cache::key(cache_parts);
    if let Some(hit) = cache::get(namespace, &ck) {
        return parse_csv(&hit);
    }
    let body = json!({
        "model": model,
        "input": [{ "content": content }],
        "dimensions": dim,
        "encoding_format": "float"
    });
    let vec = post_embedding(&key, body).await?;
    if vec.len() != dim {
        tracing::warn!(got = vec.len(), want = dim, "multimodal embedding dim mismatch");
        return None;
    }
    cache::put(namespace, &ck, &vec_csv(&vec));
    Some(vec)
}

async fn post_embedding(key: &str, body: Value) -> Option<Vec<f32>> {
    let resp = client().post(EMBED_URL).bearer_auth(key).json(&body).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::warn!(status = %resp.status(), "embedding request failed");
        return None;
    }
    let v: Value = resp.json().await.ok()?;
    let arr = v["data"][0]["embedding"].as_array()?;
    Some(arr.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect())
}

/// Embed caption text for the `embedding_text` column.
pub async fn embed_text(text: &str, dim: usize) -> Option<Vec<f32>> {
    if text.trim().is_empty() {
        return None;
    }
    if embed_mock() {
        embed_mock_vec(text, dim)
    } else {
        embed_openrouter_text(&text_model(), text, dim, "embed-text").await
    }
}

/// Embed image bytes for the `embedding_visual` column.
pub async fn embed_image(bytes: &[u8], mime: &str, dim: usize) -> Option<Vec<f32>> {
    if bytes.is_empty() {
        return None;
    }
    if embed_mock() {
        return embed_image_mock(bytes, dim);
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let data_url = format!("data:{mime};base64,{b64}");
    let content = json!([{ "type": "image_url", "image_url": { "url": data_url } }]);
    let sha = bytes_sha(bytes);
    embed_openrouter_multimodal(
        &visual_model(),
        content,
        dim,
        &[&visual_model(), &dim.to_string(), "image", &sha],
        "embed-visual",
    )
    .await
}

/// Query vector for comparing against `embedding_visual` (cross-modal in real
/// mode via the multimodal model's text path).
pub async fn embed_query_visual_space(text: &str, dim: usize) -> Option<Vec<f32>> {
    if text.trim().is_empty() {
        return None;
    }
    if embed_mock() {
        return embed_mock_vec(text, dim);
    }
    let content = json!([{ "type": "text", "text": text }]);
    embed_openrouter_multimodal(
        &visual_model(),
        content,
        dim,
        &[&visual_model(), &dim.to_string(), "query", text],
        "embed-visual-query",
    )
    .await
}

pub fn is_image_asset(a: &Asset) -> bool {
    matches!(
        a.kind,
        crate::models::AssetKind::Image
            | crate::models::AssetKind::Icon
            | crate::models::AssetKind::Illustration
            | crate::models::AssetKind::Svg
    ) || a.mime_type.as_deref().is_some_and(|m| m.starts_with("image/"))
}

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

/// The text we embed for an asset — its searchable identity.
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

/// Index (or re-index) one asset's text + visual embeddings.
pub async fn index_asset_with_bytes(
    pool: &PgPool,
    asset: &Asset,
    image_bytes: Option<&[u8]>,
) -> Result<(), AppError> {
    let caption = caption_from(asset);
    let text_vec = if caption.trim().is_empty() {
        None
    } else {
        embed_text(&caption, VISUAL_DIM).await
    };

    let visual_vec = if is_image_asset(asset) {
        if let Some(bytes) = image_bytes {
            let mime = asset.mime_type.as_deref().unwrap_or("image/png");
            embed_image(bytes, mime, VISUAL_DIM).await
        } else {
            None
        }
    } else {
        None
    };

    if text_vec.is_none() && visual_vec.is_none() {
        return Ok(());
    }

    let text_pg = text_vec.as_ref().map(|v| to_pgvector(v));
    let visual_pg = visual_vec.as_ref().map(|v| to_pgvector(v));
    let m_text = text_vec.as_ref().map(|_| model_tag_text());
    let m_visual = visual_vec.as_ref().map(|_| model_tag_visual());

    sqlx::query("DELETE FROM visual_embeddings WHERE asset_id = $1")
        .bind(asset.id)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO visual_embeddings (asset_id, embedding_text, embedding_visual, model_text, model_visual)
         VALUES ($1, $2::vector, $3::vector, $4, $5)",
    )
    .bind(asset.id)
    .bind(text_pg)
    .bind(visual_pg)
    .bind(&m_text)
    .bind(&m_visual)
    .execute(pool)
    .await?;

    let sha = image_bytes.map(bytes_sha);
    mirror::save_embedding_sidecar(
        asset.project_id,
        asset.id,
        &caption,
        asset.mime_type.as_deref(),
        sha.as_deref(),
        m_text.as_deref(),
        m_visual.as_deref(),
        text_vec.as_deref(),
        visual_vec.as_deref(),
    );

    Ok(())
}

/// Best-effort dual index + semantic context row. Never fails the caller.
pub async fn index_asset_soft(pool: &PgPool, asset: &Asset, image_bytes: Option<&[u8]>) {
    if let Err(e) = index_asset_with_bytes(pool, asset, image_bytes).await {
        tracing::warn!(error = %e, asset = %asset.id, "asset embedding index failed (non-fatal)");
    }
    let rationale = asset.prompt.as_deref().or(asset.derivation.as_deref());
    if let Some(text) = rationale {
        if let Err(e) = index_semantic(pool, asset.project_id, "asset_prompt", Some(asset.id), text).await {
            tracing::warn!(error = %e, asset = %asset.id, "asset semantic embedding failed (non-fatal)");
        }
    }
}

/// Index a text snippet into the semantic-context store.
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

#[cfg(test)]
mod tests {
    use super::{embed_image_mock, embed_mock_vec as embed_text, bytes_sha, to_pgvector, VISUAL_DIM};

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
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
    fn distinct_images_differ() {
        let a = embed_image_mock(&[1, 2, 3, 4, 5, 6, 7, 8], VISUAL_DIM).unwrap();
        let b = embed_image_mock(&[9, 8, 7, 6, 5, 4, 3, 2], VISUAL_DIM).unwrap();
        assert!(cosine(&a, &b) < 0.99);
    }

    #[test]
    fn bytes_sha_is_stable() {
        assert_eq!(bytes_sha(b"abc"), bytes_sha(b"abc"));
        assert_ne!(bytes_sha(b"abc"), bytes_sha(b"abcd"));
    }

    #[test]
    fn pgvector_text_format() {
        assert_eq!(to_pgvector(&[1.0, 2.5]), "[1.000000,2.500000]");
    }
}
