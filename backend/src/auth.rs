use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{Duration, Utc};
use rand::RngCore;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::WorkspaceRole;
use crate::AppState;

pub const COOKIE_NAME: &str = "ds_session";
const SESSION_TTL_DAYS: i64 = 30;

// ── Passwords (argon2id) ─────────────────────────────────────────────────────

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let mut salt_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|e| AppError::Internal(format!("salt: {e}")))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("hash: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

// ── Sessions ─────────────────────────────────────────────────────────────────

fn new_session_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Insert a session row and return its opaque token.
pub async fn create_session(pool: &PgPool, user_id: Uuid) -> Result<String, AppError> {
    let token = new_session_token();
    let expires_at = Utc::now() + Duration::days(SESSION_TTL_DAYS);
    sqlx::query("INSERT INTO sessions (user_id, token, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&token)
        .bind(expires_at)
        .execute(pool)
        .await?;
    Ok(token)
}

pub async fn delete_session(pool: &PgPool, token: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM sessions WHERE token = $1")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Build the session cookie. `Secure` is gated on `COOKIE_SECURE=true` so local
/// http dev works while production stays secure.
pub fn session_cookie(token: String) -> Cookie<'static> {
    Cookie::build((COOKIE_NAME, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(cookie_secure())
        .max_age(time::Duration::days(SESSION_TTL_DAYS))
        .build()
}

/// A removal cookie that clears the session client-side on logout.
pub fn clear_cookie() -> Cookie<'static> {
    Cookie::build((COOKIE_NAME, "")).path("/").build()
}

fn cookie_secure() -> bool {
    std::env::var("COOKIE_SECURE").map(|v| v == "true").unwrap_or(false)
}

// ── Authenticated-user extractor ─────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
pub struct AuthUser {
    pub id: Uuid,
    // Carried on the principal for logging / future handlers.
    #[allow(dead_code)]
    pub email: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .expect("CookieJar extraction is infallible");
        let token = jar.get(COOKIE_NAME).map(|c| c.value().to_owned());
        let token = token.ok_or(AppError::Unauthorized)?;

        sqlx::query_as::<_, AuthUser>(
            "SELECT u.id, u.email
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token = $1 AND s.expires_at > now()",
        )
        .bind(token)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::Unauthorized)
    }
}

// ── Access control ───────────────────────────────────────────────────────────

fn check_role(role: Option<WorkspaceRole>, min: WorkspaceRole) -> Result<(), AppError> {
    match role {
        // Not a member → 404 (don't reveal that the workspace exists).
        None => Err(AppError::NotFound),
        Some(r) if r >= min => Ok(()),
        Some(_) => Err(AppError::Forbidden),
    }
}

/// Require the user to be a member of `workspace_id` with at least `min` role.
pub async fn require_member(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
    min: WorkspaceRole,
) -> Result<(), AppError> {
    let role: Option<WorkspaceRole> = sqlx::query_scalar(
        "SELECT role FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    check_role(role, min)
}

/// Authorize access to a project via its owning workspace.
pub async fn require_project_access(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
    min: WorkspaceRole,
) -> Result<(), AppError> {
    let role: Option<WorkspaceRole> = sqlx::query_scalar(
        "SELECT m.role FROM projects p
         JOIN workspace_members m ON m.workspace_id = p.workspace_id
         WHERE p.id = $1 AND m.user_id = $2",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    check_role(role, min)
}

/// Authorize access to an artifact via its project's workspace.
pub async fn require_artifact_access(
    pool: &PgPool,
    artifact_id: Uuid,
    user_id: Uuid,
    min: WorkspaceRole,
) -> Result<(), AppError> {
    let role: Option<WorkspaceRole> = sqlx::query_scalar(
        "SELECT m.role FROM artifacts a
         JOIN projects p ON p.id = a.project_id
         JOIN workspace_members m ON m.workspace_id = p.workspace_id
         WHERE a.id = $1 AND m.user_id = $2",
    )
    .bind(artifact_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    check_role(role, min)
}
