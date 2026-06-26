//! Unity export packer.
//!
//! Emits a `<file>.meta` next to each texture so Unity imports it as a 2D
//! Sprite with a **stable GUID** (Unity references assets by GUID, so a fixed
//! GUID keeps any future prefab/scene refs valid across machines), plus a
//! README. The user copies the `assets/` folder into their project's `Assets/`.
//!
//! ## Scope of the `.meta` (and why it's deliberately lean)
//! A full Unity `TextureImporter` block is large and its `serializedVersion`
//! tracks the editor version, so a byte-exact match can't be produced without
//! the (licensed) editor. We ship a well-formed, minimal `.meta`: the GUID +
//! the settings that matter for a 2D sprite (`textureType: 8` = Sprite,
//! `alphaIsTransparency`, point filtering, no mipmaps). On first import Unity
//! honors these and augments the file with defaults for the rest — so the pack
//! is import-tolerant rather than version-pinned. This is format-validated, not
//! editor-verified (see the crate docs / HANDOFF).

/// A deterministic 32-hex Unity GUID for `seed` (the asset's res path). Unity
/// only needs a unique 32-char hex string; we build one from two FNV-1a passes
/// with different salts — no crypto dependency, stable across exports.
pub fn guid(seed: &str) -> String {
    fn fnv1a(salt: u8, s: &str) -> u64 {
        let mut h = 1469598103934665603u64;
        h ^= salt as u64;
        h = h.wrapping_mul(1099511628211);
        for b in s.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        h
    }
    format!("{:016x}{:016x}", fnv1a(0x01, seed), fnv1a(0x02, seed))
}

/// `.meta` contents for a texture at `asset_path` (e.g.
/// `assets/character/01-hero.png`). The GUID is derived from that path so a
/// re-export of the same asset keeps the same GUID.
pub fn texture_meta(asset_path: &str) -> String {
    format!(
        "fileFormatVersion: 2\n\
         guid: {guid}\n\
         TextureImporter:\n\
         \x20\x20serializedVersion: 12\n\
         \x20\x20textureType: 8\n\
         \x20\x20textureShape: 1\n\
         \x20\x20spriteMode: 1\n\
         \x20\x20spritePixelsToUnit: 100\n\
         \x20\x20alphaUsage: 1\n\
         \x20\x20alphaIsTransparency: 1\n\
         \x20\x20mipmaps:\n\
         \x20\x20\x20\x20enableMipMap: 0\n\
         \x20\x20textureSettings:\n\
         \x20\x20\x20\x20filterMode: 0\n\
         \x20\x20\x20\x20aniso: 1\n\
         \x20\x20\x20\x20wrapU: 1\n\
         \x20\x20\x20\x20wrapV: 1\n\
         \x20\x20userData: \n\
         \x20\x20assetBundleName: \n\
         \x20\x20assetBundleVariant: \n",
        guid = guid(asset_path)
    )
}

/// The pack README: how to drop it into a Unity project + provenance.
pub fn readme(project_name: &str, canon_version: Option<i32>, groups: &[(String, usize)]) -> String {
    let canon = canon_version.map(|v| v.to_string()).unwrap_or_else(|| "—".into());
    let mut group_lines = String::new();
    for (name, count) in groups {
        group_lines.push_str(&format!("- `assets/{name}/` — {count} asset(s)\n"));
    }
    if group_lines.is_empty() {
        group_lines.push_str("- (no assets)\n");
    }
    format!(
        "# {project_name} — Unity pack\n\n\
         Exported from CanonForge. Canon version: {canon}.\n\n\
         ## Use it\n\n\
         Copy the `assets/` folder into your Unity project's `Assets/` directory. \
         Each texture ships with a `.meta` that imports it as a **2D Sprite** \
         (point filtering, no mipmaps, alpha kept) and carries a stable GUID so \
         references stay valid. Unity fills in any remaining importer defaults on \
         first import.\n\n\
         ## Layout\n\n\
         Assets are grouped by role/tag — the same `groups[]` as `manifest.json`:\n\n\
         {group_lines}\n\
         Each `<name>.png` has a sibling `<name>.png.meta`. `manifest.json` carries \
         the full per-asset metadata (dimensions, alpha, tags, status).\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_is_32_hex_and_deterministic() {
        let g = guid("assets/character/01-hero.png");
        assert_eq!(g.len(), 32);
        assert!(g.chars().all(|c| c.is_ascii_hexdigit()));
        // Stable across calls, distinct per path.
        assert_eq!(g, guid("assets/character/01-hero.png"));
        assert_ne!(g, guid("assets/character/02-hero.png"));
    }

    #[test]
    fn texture_meta_is_a_2d_sprite() {
        let s = texture_meta("assets/prop/01-crate.png");
        assert!(s.starts_with("fileFormatVersion: 2\n"));
        assert!(s.contains("TextureImporter:"));
        // textureType 8 == Sprite (2D and UI); alpha preserved.
        assert!(s.contains("textureType: 8"));
        assert!(s.contains("alphaIsTransparency: 1"));
        // The shipped GUID matches the deterministic helper.
        assert!(s.contains(&format!("guid: {}", guid("assets/prop/01-crate.png"))));
    }

    #[test]
    fn readme_lists_groups_and_canon() {
        let groups = vec![("character".to_string(), 2)];
        let s = readme("Forest Game", Some(7), &groups);
        assert!(s.contains("Unity pack"));
        assert!(s.contains("Canon version: 7"));
        assert!(s.contains("`assets/character/` — 2 asset(s)"));
        assert!(s.contains("Assets/"));
    }
}
