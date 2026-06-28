//! Optional on-disk mirror of asset bytes. When `ASSET_MIRROR_DIR` is set, every
//! generated / derived / uploaded asset is *also* written as a plain file at
//! `<dir>/<project_id>/<asset_id>.<ext>` — a browsable, backup-friendly copy
//! alongside object storage (MinIO/S3) and the DB. Best-effort: a write failure
//! is logged, never fatal, and never blocks the request. Inert when the env var
//! is unset (so tests / CI write nothing).

use std::path::PathBuf;

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

/// Write `bytes` to `<ASSET_MIRROR_DIR>/<project_id>/<asset_id>.<ext>` if the
/// mirror is enabled. No-op otherwise.
pub fn save(project_id: Uuid, asset_id: Uuid, mime: &str, bytes: &[u8]) {
    let Some(base) = dir() else { return };
    let folder = base.join(project_id.to_string());
    if let Err(e) = std::fs::create_dir_all(&folder) {
        tracing::warn!(error = %e, "asset mirror: could not create dir");
        return;
    }
    let path = folder.join(format!("{asset_id}.{}", ext_for(mime)));
    if let Err(e) = std::fs::write(&path, bytes) {
        tracing::warn!(error = %e, path = %path.display(), "asset mirror: write failed");
    }
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
