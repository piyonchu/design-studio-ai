//! A tiny content-addressed disk cache for AI outputs. Keyed by a hash of the
//! request, so an identical call is served from disk and never re-spends. Lives
//! under `AI_CACHE_DIR` (default `./.ai-cache`, gitignored) — a durable record
//! of everything we generated, reusable across runs.

use std::path::PathBuf;

fn dir() -> PathBuf {
    std::env::var("AI_CACHE_DIR").unwrap_or_else(|_| "./.ai-cache".to_string()).into()
}

/// FNV-1a 64-bit over the parts → hex. Collisions are astronomically unlikely
/// for a cache; a miss just re-computes, so this is safe.
pub fn key(parts: &[&str]) -> String {
    let mut h = 1469598103934665603u64;
    for p in parts {
        for b in p.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        h ^= 0xff;
        h = h.wrapping_mul(1099511628211);
    }
    format!("{h:016x}")
}

/// Read a cached value, or None on miss / unreadable.
pub fn get(namespace: &str, key: &str) -> Option<String> {
    std::fs::read_to_string(dir().join(namespace).join(format!("{key}.txt"))).ok()
}

/// Write a value to the cache (best-effort: a failure just means no caching).
pub fn put(namespace: &str, key: &str, value: &str) {
    let d = dir().join(namespace);
    if std::fs::create_dir_all(&d).is_ok() {
        let _ = std::fs::write(d.join(format!("{key}.txt")), value);
    }
}
