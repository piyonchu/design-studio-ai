//! Godot 4 export packer.
//!
//! Turns the generic export pack into a drop-in Godot project: the same
//! `assets/<group>/<file>` textures, plus a `<file>.import` next to each so the
//! engine imports them with sprite-friendly settings, a minimal `project.godot`
//! so the bundle opens directly as a project, and a README.
//!
//! ## Why the `.import` files are deliberately minimal
//! A Godot `.import` has two halves: the *settings* (`[params]`) and the
//! *compiled-cache pointer* (`[remap] path`/`uid`/`dest_files`, which reference
//! machine-local files under `res://.godot/imported/`). We ship only the
//! settings + the source path. On first import (opening the project, or
//! `godot --headless --import`) Godot fills in the cache pointer and assigns a
//! UID, while **keeping** the `[params]` we shipped. This is self-healing — the
//! pack never carries a stale absolute cache path or a UID that could collide —
//! and dependency-free (no MD5/UID encoding to get byte-exact against Godot).
//!
//! The chosen params suit 2D sprites with transparency: lossless (uncompressed
//! VRAM) so alpha edges stay crisp, no mipmaps, and alpha-border fix on to avoid
//! dark halos when the sprite is scaled.

/// The `[params]` block shared by every texture import — 2D-sprite defaults.
const TEXTURE_PARAMS: &str = "\
compress/mode=0
compress/high_quality=false
compress/lossy_quality=0.7
compress/hdr_compression=1
compress/normal_map=0
compress/channel_pack=0
mipmaps/generate=false
mipmaps/limit=-1
roughness/mode=0
roughness/src_normal=\"\"
process/fix_alpha_border=true
process/premult_alpha=false
process/normal_map_invert_y=false
process/hdr_as_srgb=false
process/hdr_clamp_exponent=true
process/size_limit=0
detect_3d/compress_to=1";

/// `.import` contents for a texture at `res://<asset_path>` (e.g.
/// `assets/character/01-hero.png`). We omit the `[remap] path`/`uid`/`dest_files`
/// cache pointer on purpose — Godot regenerates it on first import and preserves
/// the `[params]` below. See the module docs.
pub fn texture_import(asset_path: &str) -> String {
    format!(
        "[remap]\n\nimporter=\"texture\"\ntype=\"CompressedTexture2D\"\n\n\
         [deps]\n\nsource_file=\"res://{asset_path}\"\n\n\
         [params]\n\n{TEXTURE_PARAMS}\n"
    )
}

/// A minimal Godot 4 `project.godot` so the pack opens directly as a project
/// (and so `godot --headless --import` can finalize the textures). `config_version=5`
/// is the Godot 4.x project format; the engine version is intentionally not
/// pinned so any 4.x opens it without a downgrade prompt.
pub fn project_godot(project_name: &str) -> String {
    let safe = project_name.replace('"', "'");
    format!(
        "; CanonForge export — open this folder in Godot 4, or copy `assets/` into your project.\n\
         config_version=5\n\n[application]\n\nconfig/name=\"{safe} (CanonForge pack)\"\n"
    )
}

/// The pack README: layout + the two ways to use it + provenance.
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
        "# {project_name} — Godot pack\n\n\
         Exported from CanonForge. Canon version: {canon}.\n\n\
         ## Use it\n\n\
         **Option A — open as a project:** open this folder in Godot 4. The engine \
         imports every texture on first open (sprite-friendly settings are preset in the \
         `.import` files).\n\n\
         **Option B — drop into an existing project:** copy the `assets/` folder into your \
         project's `res://`. Godot imports the textures (and their preset `.import` settings) \
         on the next editor focus.\n\n\
         Import is also scriptable headlessly: `godot --headless --import` from this folder.\n\n\
         ## Layout\n\n\
         Assets are grouped by role/tag — the same `groups[]` as `manifest.json`:\n\n\
         {group_lines}\n\
         Each `<name>.png` has a sibling `<name>.png.import` (Godot import settings). \
         `manifest.json` carries the full per-asset metadata (dimensions, alpha, tags, status).\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_import_is_wellformed_and_minimal() {
        let s = texture_import("assets/character/01-hero.png");
        // Importer + type so Godot knows how to (re)import.
        assert!(s.contains("importer=\"texture\""));
        assert!(s.contains("type=\"CompressedTexture2D\""));
        // Source path is the res:// path of the shipped png.
        assert!(s.contains("source_file=\"res://assets/character/01-hero.png\""));
        // Our 2D settings are present.
        assert!(s.contains("mipmaps/generate=false"));
        assert!(s.contains("process/fix_alpha_border=true"));
        // We intentionally DON'T ship the machine-local compiled-cache pointer;
        // Godot regenerates it (and the UID) on first import.
        assert!(!s.contains("uid://"));
        assert!(!s.contains(".godot/imported/"));
        assert!(!s.contains("dest_files="));
    }

    #[test]
    fn project_godot_is_godot4_and_escapes_quotes() {
        let s = project_godot("My \"Cool\" Game");
        assert!(s.contains("config_version=5"));
        assert!(s.contains("[application]"));
        // A stray quote in the name can't break the config string.
        assert!(!s.contains("\"My \"Cool\""));
        assert!(s.contains("My 'Cool' Game"));
    }

    #[test]
    fn readme_lists_groups_and_canon() {
        let groups = vec![("character".to_string(), 2), ("prop".to_string(), 1)];
        let s = readme("Forest Game", Some(4), &groups);
        assert!(s.contains("Forest Game"));
        assert!(s.contains("Canon version: 4"));
        assert!(s.contains("`assets/character/` — 2 asset(s)"));
        assert!(s.contains("`assets/prop/` — 1 asset(s)"));
        assert!(s.contains("godot --headless --import"));
    }

    #[test]
    fn readme_handles_no_canon_and_no_groups() {
        let s = readme("Empty", None, &[]);
        assert!(s.contains("Canon version: —"));
        assert!(s.contains("(no assets)"));
    }
}
