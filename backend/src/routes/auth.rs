use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::CookieJar;

use crate::auth::{
    self, clear_cookie, create_session, hash_password, session_cookie, verify_password, AuthUser,
    COOKIE_NAME,
};
use crate::error::AppError;
use crate::models::{
    LoginRequest, SignupRequest, SignupResponse, User, UserCredentials, Workspace,
};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
}

/// Create a user + a default workspace (caller becomes owner), open a session.
async fn signup(
    State(state): State<AppState>,
    jar: CookieJar,
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
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2)
         RETURNING id, email, created_at",
    )
    .bind(&email)
    .bind(&password_hash)
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

    let user = sqlx::query_as::<_, User>("SELECT id, email, created_at FROM users WHERE id = $1")
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
    let user = sqlx::query_as::<_, User>("SELECT id, email, created_at FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(user))
}
