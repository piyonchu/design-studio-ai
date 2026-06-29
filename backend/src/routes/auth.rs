use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::CookieJar;
use tower_governor::GovernorLayer;

use crate::auth::{
    self, clear_cookie, create_session, hash_password, session_cookie, verify_password, AuthUser,
    COOKIE_NAME,
};
use crate::error::AppError;
use crate::models::{
    LoginRequest, SignupRequest, SignupResponse, UpdateProfile, User, UserCredentials, Workspace,
};
use crate::ratelimit;
use crate::turnstile::TurnstileGuard;
use crate::AppState;

pub fn router() -> Router<AppState> {
    // Brute-force targets get the stricter per-IP limit (plus Turnstile);
    // session-bound routes (me/logout) rely on the global limit only.
    let sensitive = Router::new()
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .layer(GovernorLayer {
            config: ratelimit::auth_config(),
        });

    let session = Router::new()
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me).patch(update_me));

    sensitive.merge(session)
}

/// Create a user + a default workspace (caller becomes owner), open a session.
async fn signup(
    State(state): State<AppState>,
    jar: CookieJar,
    _guard: TurnstileGuard,
    Json(body): Json<SignupRequest>,
) -> Result<(StatusCode, CookieJar, Json<SignupResponse>), AppError> {
    let email = body.email.trim().to_lowercase();
    if email.is_empty() || body.password.len() < 8 {
        return Err(AppError::BadRequest(
            "email required and password must be at least 8 characters".into(),
        ));
    }
    let password_hash = hash_password(&body.password)?;
    let ws_name = body
        .workspace_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "My Workspace".to_string());

    let mut tx = state.pool.begin().await?;

    // Unique index on lower(email) enforces no duplicates → map 23505 to 400.
    // Default the display name to the email's local part (editable later).
    let default_name = email.split('@').next().unwrap_or(&email).to_string();
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash, display_name) VALUES ($1, $2, $3)
         RETURNING id, email, display_name, created_at",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(&default_name)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            AppError::BadRequest("email already registered".into())
        }
        _ => AppError::from(e),
    })?;

    let workspace = sqlx::query_as::<_, Workspace>(
        "INSERT INTO workspaces (name) VALUES ($1) RETURNING id, name, created_at",
    )
    .bind(&ws_name)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(workspace.id)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let token = create_session(&state.pool, user.id).await?;
    let jar = jar.add(session_cookie(token));
    Ok((
        StatusCode::CREATED,
        jar,
        Json(SignupResponse { user, workspace }),
    ))
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    _guard: TurnstileGuard,
    Json(body): Json<LoginRequest>,
) -> Result<(CookieJar, Json<User>), AppError> {
    let email = body.email.trim().to_lowercase();
    let creds = sqlx::query_as::<_, UserCredentials>(
        "SELECT id, password_hash FROM users WHERE lower(email) = $1",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?;

    // Generic 401 whether the user is missing or the password is wrong.
    let creds = creds.ok_or(AppError::Unauthorized)?;
    if !verify_password(&body.password, &creds.password_hash) {
        return Err(AppError::Unauthorized);
    }

    let user = sqlx::query_as::<_, User>(
        "SELECT id, email, display_name, created_at FROM users WHERE id = $1",
    )
    .bind(creds.id)
    .fetch_one(&state.pool)
    .await?;

    let token = create_session(&state.pool, user.id).await?;
    let jar = jar.add(session_cookie(token));
    Ok((jar, Json(user)))
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), AppError> {
    if let Some(token) = jar.get(COOKIE_NAME).map(|c| c.value().to_owned()) {
        auth::delete_session(&state.pool, &token).await?;
    }
    Ok((jar.remove(clear_cookie()), StatusCode::NO_CONTENT))
}

async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<User>, AppError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, email, display_name, created_at FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(user))
}

/// Update the signed-in user's profile (display name). An empty/blank name
/// clears it (UI falls back to the email).
async fn update_me(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<UpdateProfile>,
) -> Result<Json<User>, AppError> {
    let name = body
        .display_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let user = sqlx::query_as::<_, User>(
        "UPDATE users SET display_name = $2 WHERE id = $1
         RETURNING id, email, display_name, created_at",
    )
    .bind(user.id)
    .bind(name)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(user))
}
