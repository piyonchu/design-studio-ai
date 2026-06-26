//! Vertical-adapter registry — the backend half of a "vertical pack". Each
//! vertical owns the prompt rule its generations are framed by (`render_hint`).
//! The frontend owns the UX half (derive presets + canon fields) in
//! `frontend/src/app/verticals.ts`, keyed by the same `key`. Adding a vertical
//! = one row here + one entry there.

pub struct Vertical {
    pub key: &'static str,
    /// Human label — surfaced by `all()` for a future `GET /verticals`; the web
    /// UI currently carries its own labels in `verticals.ts`.
    #[allow(dead_code)]
    pub label: &'static str,
    /// Appended to every generate/derive prompt — the vertical's framing
    /// (e.g. an isolated game sprite vs a webtoon panel cutout).
    pub render_hint: &'static str,
    /// Engine the vertical can emit an import-ready export pack for, if any.
    /// This is the per-vertical export-adapter hook: the export route consults
    /// it to decide which engine packer (`crate::export`) a project can target.
    pub engine: Option<Engine>,
}

/// A game/app engine a vertical can produce an import-ready asset pack for.
/// Adding a target = a variant here + a packer in `crate::export`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Engine {
    Godot,
}

impl Engine {
    /// Stable wire tag used by the export request + the frontend.
    pub fn tag(self) -> &'static str {
        match self {
            Engine::Godot => "godot",
        }
    }

    pub fn from_tag(s: &str) -> Option<Engine> {
        match s {
            "godot" => Some(Engine::Godot),
            _ => None,
        }
    }
}

/// The known verticals. The first entry is the default fallback.
const VERTICALS: &[Vertical] = &[
    Vertical {
        key: "game_2d",
        label: "Game (2D)",
        render_hint: "One centered isolated asset, transparent background.",
        // 2D game assets drop straight into a Godot project as sprites.
        engine: Some(Engine::Godot),
    },
    Vertical {
        key: "manhwa",
        label: "Manhwa / Webtoon",
        render_hint:
            "A single character or element as a clean cutout on a transparent background, webtoon panel-ready.",
        engine: None,
    },
    Vertical {
        key: "illustration",
        label: "Illustration",
        render_hint: "A single polished illustration subject, clean composition on a transparent background.",
        engine: None,
    },
];

/// Look up a vertical by key, falling back to the default (`game_2d`) for an
/// unknown key so prompt compilation never fails.
pub fn get(key: &str) -> &'static Vertical {
    VERTICALS.iter().find(|v| v.key == key).unwrap_or(&VERTICALS[0])
}

/// Whether `key` is a registered vertical (used to validate project creation).
pub fn is_known(key: &str) -> bool {
    VERTICALS.iter().any(|v| v.key == key)
}

#[allow(dead_code)]
pub fn all() -> &'static [Vertical] {
    VERTICALS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lookup_and_fallback() {
        assert_eq!(get("manhwa").key, "manhwa");
        assert_eq!(get("illustration").key, "illustration");
        assert_eq!(get("bogus").key, "game_2d"); // unknown → default
        assert!(is_known("game_2d") && is_known("illustration"));
        assert!(!is_known("nope"));
        assert!(all().len() >= 3);
    }

    #[test]
    fn engine_hook_and_tags() {
        // game_2d can emit a Godot pack; other verticals declare no engine yet.
        assert_eq!(get("game_2d").engine, Some(Engine::Godot));
        assert_eq!(get("manhwa").engine, None);
        assert_eq!(get("illustration").engine, None);
        // Wire tag round-trips.
        assert_eq!(Engine::Godot.tag(), "godot");
        assert_eq!(Engine::from_tag("godot"), Some(Engine::Godot));
        assert_eq!(Engine::from_tag("unity"), None);
    }
}
