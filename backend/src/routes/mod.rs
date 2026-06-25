mod ai;
mod artifacts;
mod assets;
mod auth;
mod projects;
mod workspaces;

use axum::Router;

use crate::AppState;

/// All API routes, merged into one router shared with the app's `AppState`.
/// (The stricter auth rate limit is applied inside `auth::router()`.)
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(workspaces::router())
        .merge(projects::router())
        .merge(artifacts::router())
        .merge(ai::router())
        .merge(assets::router())
}
