//! Optional on-disk mirror of asset bytes + embedding sidecars. When
//! `ASSET_MIRROR_DIR` is set, every generated / derived / uploaded image is also
//! written as a plain file at `<dir>/<project_id>/<asset_id>.<ext>` with a
//! sibling `<asset_id>.embed.json` holding text + visual vectors for deploy
//! migration (import into production DB without re-embedding). Best-effort:
//! failures are logged, never fatal. Inert when the env var is unset.

use std::path::PathBuf;

use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

fn dir() -> Option<PathBuf> {
    std::env::var("ASSET_MIRROR_DIR")
        .ok()
        .filter(|d| !d.trim().is_empty())
        .map(PathBuf::from)
}

/// File extension for a stored mime type (mirrors the export packer's mapping).
fn ext_for(mime: &str) -> &'static str {
    let m = mime.to_ascii_lowercase();
    if m.contains("png") {
        "png"
    } else if m.contains("jpeg") || m.contains("jpg") {
        "jpg"
    } else if m.contains("webp") {
        "webp"
    } else if m.contains("svg") {
        "svg"
    } else if m.contains("wav") {
        "wav"
    } else if m.contains("mpeg") || m.contains("mp3") {
        "mp3"
    } else {
        "bin"
    }
}

/// Path to a mirrored asset file, if the mirror is enabled.
pub fn asset_path(project_id: Uuid, asset_id: Uuid, mime: &str) -> Option<PathBuf> {
    dir().map(|base| base.join(project_id.to_string()).join(format!("{asset_id}.{}", ext_for(mime))))
}

/// Write `bytes` to `<ASSET_MIRROR_DIR>/<project_id>/<asset_id>.<ext>` if the
/// mirror is enabled. No-op otherwise.
pub fn save(project_id: Uuid, asset_id: Uuid, mime: &str, bytes: &[u8]) {
    let Some(path) = asset_path(project_id, asset_id, mime) else { return };
    if let Some(folder) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(folder) {
            tracing::warn!(error = %e, "asset mirror: could not create dir");
            return;
        }
    }
    if let Err(e) = std::fs::write(&path, bytes) {
        tracing::warn!(error = %e, path = %path.display(), "asset mirror: write failed");
    }
}

/// Read mirrored bytes for an asset when the mirror is enabled.
pub fn read(project_id: Uuid, asset_id: Uuid, mime: &str) -> Option<Vec<u8>> {
    let path = asset_path(project_id, asset_id, mime)?;
    std::fs::read(path).ok()
}

/// Try to locate a mirrored file when mime is unknown (any known extension).
pub fn read_any(project_id: Uuid, asset_id: Uuid) -> Option<(Vec<u8>, String)> {
    let base = dir()?.join(project_id.to_string());
    for ext in ["png", "jpg", "webp", "svg", "bin"] {
        let path = base.join(format!("{asset_id}.{ext}"));
        if path.is_file() {
            let bytes = std::fs::read(&path).ok()?;
            let mime = match ext {
                "png" => "image/png",
                "jpg" => "image/jpeg",
                "webp" => "image/webp",
                "svg" => "image/svg+xml",
                _ => "application/octet-stream",
            };
            return Some((bytes, mime.to_string()));
        }
    }
    None
}

#[derive(Serialize)]
struct EmbedSidecar {
    asset_id: Uuid,
    caption: String,
    mime: Option<String>,
    bytes_sha: Option<String>,
    model_text: Option<String>,
    model_visual: Option<String>,
    embedding_text: Option<Vec<f32>>,
    embedding_visual: Option<Vec<f32>>,
    cached_at: String,
}

/// Persist embedding vectors beside the mirrored image for deploy migration.
pub fn save_embedding_sidecar(
    project_id: Uuid,
    asset_id: Uuid,
    caption: &str,
    mime: Option<&str>,
    bytes_sha: Option<&str>,
    model_text: Option<&str>,
    model_visual: Option<&str>,
    embedding_text: Option<&[f32]>,
    embedding_visual: Option<&[f32]>,
) {
    let Some(base) = dir() else { return };
    let folder = base.join(project_id.to_string());
    if let Err(e) = std::fs::create_dir_all(&folder) {
        tracing::warn!(error = %e, "asset mirror: could not create dir for sidecar");
        return;
    }
    let path = folder.join(format!("{asset_id}.embed.json"));
    let sidecar = EmbedSidecar {
        asset_id,
        caption: caption.to_string(),
        mime: mime.map(str::to_string),
        bytes_sha: bytes_sha.map(str::to_string),
        model_text: model_text.map(str::to_string),
        model_visual: model_visual.map(str::to_string),
        embedding_text: embedding_text.map(|v| v.to_vec()),
        embedding_visual: embedding_visual.map(|v| v.to_vec()),
        cached_at: Utc::now().to_rfc3339(),
    };
    match serde_json::to_string_pretty(&sidecar) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                tracing::warn!(error = %e, path = %path.display(), "asset mirror: sidecar write failed");
            }
        }
        Err(e) => tracing::warn!(error = %e, "asset mirror: sidecar serialize failed"),
    }
}

/// Load a previously mirrored embedding sidecar (deploy import / backfill).
pub fn read_embedding_sidecar(project_id: Uuid, asset_id: Uuid) -> Option<EmbedSidecarOut> {
    let path = dir()?.join(project_id.to_string()).join(format!("{asset_id}.embed.json"));
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[derive(Debug, serde::Deserialize)]
pub struct EmbedSidecarOut {
    pub caption: String,
    pub mime: Option<String>,
    pub model_text: Option<String>,
    pub model_visual: Option<String>,
    pub embedding_text: Option<Vec<f32>>,
    pub embedding_visual: Option<Vec<f32>>,
}

#[cfg(test)]
mod tests {
    use super::ext_for;

    #[test]
    fn ext_for_maps_known_mimes() {
        assert_eq!(ext_for("image/png"), "png");
        assert_eq!(ext_for("image/jpeg"), "jpg");
        assert_eq!(ext_for("image/webp"), "webp");
        assert_eq!(ext_for("image/svg+xml"), "svg");
        assert_eq!(ext_for("audio/wav"), "wav");
        assert_eq!(ext_for("application/octet-stream"), "bin");
    }
}
