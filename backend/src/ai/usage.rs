//! OpenRouter key budget ("how much credit is left on the shared dev key").
//!
//! Surfaces the **per-key** budget from `GET /api/v1/auth/key`
//! (`limit_remaining` / `usage` against `limit`) — not the account-wide
//! `/credits` — since the shared key is what dev spend draws down.
//!
//! Mock-first like the rest of the AI layer: with no `OPENROUTER_API_KEY` (or
//! `USAGE_MOCK=true`) it returns a placeholder so dev/CI need no network. The
//! live value is cached for [`TTL`] so page loads don't hammer OpenRouter.
//!
//! This is intentionally a thin, swappable seam: today it shows the raw key
//! budget for dev visibility; it can later become per-workspace quotas/tiers
//! without touching callers.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::Mutex;

const URL: &str = "https://openrouter.ai/api/v1/auth/key";
const TTL: Duration = Duration::from_secs(60);
const TIMEOUT_SECS: u64 = 12;

/// The shared key's remaining budget. `source` is `"openrouter"` (live),
/// `"mock"` (no key / forced), or `"stale"` (a fetch failed; last value reused).
#[derive(Clone, Serialize)]
pub struct Usage {
    /// Currency-agnostic credit remaining on the key.
    pub remaining: f64,
    /// Credit spent on the key so far.
    pub usage: f64,
    /// The key's hard limit, if one is set.
    pub limit: Option<f64>,
    pub source: &'static str,
}

fn mock_enabled() -> bool {
    std::env::var("USAGE_MOCK").map(|v| v.trim().eq_ignore_ascii_case("true")).unwrap_or(false)
}

fn api_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.trim().is_empty())
}

fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("usage http client")
    })
}

fn cache() -> &'static Mutex<Option<(Instant, Usage)>> {
    static CACHE: OnceLock<Mutex<Option<(Instant, Usage)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn mock() -> Usage {
    Usage { remaining: 9.5, usage: 0.5, limit: Some(10.0), source: "mock" }
}

/// Parse `{ "data": { "limit_remaining", "usage", "limit" } }`.
fn parse(body: &serde_json::Value) -> Option<Usage> {
    let d = body.get("data")?;
    Some(Usage {
        remaining: d.get("limit_remaining").and_then(|v| v.as_f64())?,
        usage: d.get("usage").and_then(|v| v.as_f64()).unwrap_or(0.0),
        limit: d.get("limit").and_then(|v| v.as_f64()),
        source: "openrouter",
    })
}

/// The shared key's budget, cached for [`TTL`]. Never errors: falls back to the
/// last good value (`source:"stale"`) or the mock so the UI always renders.
pub async fn key_balance() -> Usage {
    if mock_enabled() {
        return mock();
    }
    let Some(key) = api_key() else { return mock() };

    let mut slot = cache().lock().await;
    if let Some((at, ref u)) = *slot {
        if at.elapsed() < TTL {
            return u.clone();
        }
    }

    let fetched = async {
        let resp = client().get(URL).bearer_auth(&key).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        parse(&resp.json::<serde_json::Value>().await.ok()?)
    }
    .await;

    match fetched {
        Some(u) => {
            *slot = Some((Instant::now(), u.clone()));
            u
        }
        // Reuse the last good value if we have one; else mock.
        None => match slot.as_ref() {
            Some((_, prev)) => Usage { source: "stale", ..prev.clone() },
            None => mock(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_auth_key_shape() {
        let body = serde_json::json!({
            "data": { "label": "sk-...", "limit": 10, "usage": 0.47, "limit_remaining": 9.53 }
        });
        let u = parse(&body).expect("parse");
        assert_eq!(u.remaining, 9.53);
        assert_eq!(u.usage, 0.47);
        assert_eq!(u.limit, Some(10.0));
        assert_eq!(u.source, "openrouter");
    }

    #[test]
    fn missing_remaining_is_none() {
        assert!(parse(&serde_json::json!({ "data": { "usage": 1.0 } })).is_none());
        assert!(parse(&serde_json::json!({ "nope": true })).is_none());
    }

    #[test]
    fn mock_has_sane_shape() {
        let m = mock();
        assert_eq!(m.source, "mock");
        assert!(m.remaining > 0.0 && m.limit == Some(10.0));
    }
}
