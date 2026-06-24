//! Per-IP rate limiting via `tower_governor`.
//!
//! Two tiers: a generous global limit on the whole API, and a stricter limit on
//! the auth endpoints to blunt brute-force / credential-stuffing. Keyed by the
//! client IP (`SmartIpKeyExtractor` honors `X-Forwarded-For`/`X-Real-IP` behind a
//! proxy, falling back to the peer IP from `ConnectInfo`).
//!
//! NOTE: the store is in-memory, so on multi-instance deployments the effective
//! limit is per-replica. A Redis-backed store is the future upgrade.

use std::sync::Arc;
use std::time::Duration;

use governor::middleware::NoOpMiddleware;
use tower_governor::governor::{GovernorConfig, GovernorConfigBuilder};
use tower_governor::key_extractor::SmartIpKeyExtractor;

pub type IpGovernorConfig = GovernorConfig<SmartIpKeyExtractor, NoOpMiddleware>;

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn build(period: Duration, burst: u32) -> Arc<IpGovernorConfig> {
    let config = GovernorConfigBuilder::default()
        .period(period)
        .burst_size(burst)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .expect("valid governor config");
    Arc::new(config)
}

/// Global limit: replenish one cell every `1000/RATE_LIMIT_RPS` ms, up to
/// `RATE_LIMIT_BURST` outstanding (defaults ≈ 30 rps, burst 60).
pub fn global_config() -> Arc<IpGovernorConfig> {
    let rps = env_u64("RATE_LIMIT_RPS", 30).max(1);
    let burst = env_u64("RATE_LIMIT_BURST", 60).max(1) as u32;
    build(Duration::from_millis(1000 / rps), burst)
}

/// Auth limit: one cell every `AUTH_RATE_LIMIT_PERIOD_SECS`, up to
/// `AUTH_RATE_LIMIT_BURST` (defaults: burst 5, then 1 every 6s).
pub fn auth_config() -> Arc<IpGovernorConfig> {
    let period = env_u64("AUTH_RATE_LIMIT_PERIOD_SECS", 6).max(1);
    let burst = env_u64("AUTH_RATE_LIMIT_BURST", 10).max(1) as u32;
    build(Duration::from_secs(period), burst)
}

/// One-line summary for startup logging.
pub fn describe() -> String {
    format!(
        "global={}rps/burst{}, auth=1per{}s/burst{}",
        env_u64("RATE_LIMIT_RPS", 30).max(1),
        env_u64("RATE_LIMIT_BURST", 60).max(1),
        env_u64("AUTH_RATE_LIMIT_PERIOD_SECS", 6).max(1),
        env_u64("AUTH_RATE_LIMIT_BURST", 10).max(1),
    )
}
