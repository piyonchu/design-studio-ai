//! Cost guardrail: refuse generation that would drain the shared AI key or let
//! one workspace hog the pool — enforced *before* any spend, next to the
//! `moderation::check_prompt` gate so every entry point (sync generate, async
//! job worker, derive, audio) is covered.
//!
//! Two independent limits, both **tunable at runtime via env** (change the var
//! + restart — no recompile):
//!   - `GUARDRAIL_MIN_CREDIT_USD` (default 0.50) — pause generation when the
//!     shared key's remaining credit drops below this floor.
//!   - `GUARDRAIL_DAILY_GEN_CAP` (default 100) — max generation-produced assets
//!     per workspace over a rolling 24h.
//!
//! A real per-workspace dollar ledger (price each generation, bill quotas) can
//! replace the count-based cap later behind this same `check_can_spend` seam.

use uuid::Uuid;

use crate::ai::usage;
use crate::error::AppError;
use crate::AppState;

/// Default floor / cap if the env vars are unset or unparseable.
const DEFAULT_MIN_CREDIT_USD: f64 = 0.50;
const DEFAULT_DAILY_GEN_CAP: i64 = 100;

fn min_credit_usd() -> f64 {
    std::env::var("GUARDRAIL_MIN_CREDIT_USD")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_MIN_CREDIT_USD)
}

fn daily_cap() -> i64 {
    std::env::var("GUARDRAIL_DAILY_GEN_CAP")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_DAILY_GEN_CAP)
}

/// Reject a generation that would breach the guardrail, before spending.
/// `count` = how many assets this call will produce.
pub async fn check_can_spend(
    state: &AppState,
    project_id: Uuid,
    count: u32,
) -> Result<(), AppError> {
    // 1. Shared-key credit floor (global drain stop). key_balance() is cached;
    //    mock mode returns a high balance, so this never blocks free dev/CI.
    let floor = min_credit_usd();
    let bal = usage::key_balance().await;
    if bal.source != "mock" && bal.remaining < floor {
        return Err(AppError::ServiceUnavailable(format!(
            "Shared AI credit is below the ${floor:.2} floor — generation is paused. \
             Top up the OpenRouter key, or lower GUARDRAIL_MIN_CREDIT_USD."
        )));
    }

    // 2. Per-workspace rolling-24h cap (fairness). Count generation-produced
    //    assets (seeded / derived — uploads cost nothing) for the workspace.
    let cap = daily_cap();
    let used: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM assets a
         JOIN projects p ON p.id = a.project_id
         WHERE p.workspace_id = (SELECT workspace_id FROM projects WHERE id = $1)
           AND a.created_at > now() - interval '24 hours'
           AND a.source_kind IN ('seeded', 'derived')",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    if used + count as i64 > cap {
        return Err(AppError::TooManyRequests(format!(
            "Daily generation limit reached ({cap}/day for this workspace) — \
             {used} used in the last 24h. It frees up on a rolling window."
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{daily_cap, min_credit_usd, DEFAULT_DAILY_GEN_CAP, DEFAULT_MIN_CREDIT_USD};

    // Env-driven config: unset → defaults (other tests may set the vars, so only
    // assert the parse/fallback shape, not a specific global value).
    #[test]
    fn config_has_sane_defaults() {
        assert_eq!(DEFAULT_MIN_CREDIT_USD, 0.50);
        assert_eq!(DEFAULT_DAILY_GEN_CAP, 100);
        // Reading is infallible (falls back to the default) regardless of env.
        assert!(min_credit_usd() >= 0.0);
        assert!(daily_cap() >= 0);
    }
}
