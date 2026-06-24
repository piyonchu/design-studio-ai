//! Cloudflare Turnstile bot protection for the auth endpoints.
//!
//! Clients solve the Turnstile widget and send the resulting token in the
//! `cf-turnstile-response` header; the server verifies it against Cloudflare's
//! siteverify API. If `TURNSTILE_SECRET_KEY` is unset (local dev), verification
//! is bypassed with a one-time warning so the app and smoke test still work.

use std::sync::Once;

use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use serde::Deserialize;

use crate::error::AppError;
use crate::AppState;

const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const TOKEN_HEADER: &str = "cf-turnstile-response";

static DEV_WARN: Once = Once::new();

#[derive(Debug, Deserialize)]
struct SiteVerifyResponse {
    success: bool,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

fn secret() -> Option<String> {
    std::env::var("TURNSTILE_SECRET_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Verify a Turnstile token. Returns Ok in dev mode (no secret configured).
pub async fn verify(token: &str, remote_ip: Option<String>) -> Result<(), AppError> {
    let Some(secret) = secret() else {
        DEV_WARN.call_once(|| {
            tracing::warn!("TURNSTILE_SECRET_KEY unset — bot protection disabled (dev mode)");
        });
        return Ok(());
    };

    if token.is_empty() {
        return Err(AppError::BadRequest("missing Turnstile token".into()));
    }

    let mut form = vec![("secret", secret), ("response", token.to_string())];
    if let Some(ip) = remote_ip {
        form.push(("remoteip", ip));
    }

    let resp = reqwest::Client::new()
        .post(SITEVERIFY_URL)
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("turnstile request: {e}")))?;
    let body: SiteVerifyResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("turnstile parse: {e}")))?;

    if body.success {
        Ok(())
    } else {
        tracing::warn!(?body.error_codes, "turnstile verification failed");
        Err(AppError::BadRequest("failed bot check".into()))
    }
}

/// Extractor that gates a handler behind Turnstile verification.
pub struct TurnstileGuard;

#[async_trait]
impl FromRequestParts<AppState> for TurnstileGuard {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &AppState) -> Result<Self, AppError> {
        let token = parts
            .headers
            .get(TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let remote_ip = parts
            .headers
            .get("cf-connecting-ip")
            .or_else(|| parts.headers.get("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_owned());

        verify(&token, remote_ip).await?;
        Ok(TurnstileGuard)
    }
}
