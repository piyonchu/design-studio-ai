mod artifacts;
mod auth;
mod projects;
mod workspaces;

use axum::Router;

use crate::AppState;

/// All API routes, merged into one router shared with the app's `AppState`.
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(workspaces::router())
        .merge(projects::router())
        .merge(artifacts::router())
}
