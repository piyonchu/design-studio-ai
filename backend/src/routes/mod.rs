mod activity;
mod assets;
mod audio;
mod auth;
mod canon;
mod collections;
mod comments;
mod context;
mod export;
mod lineage;
mod projects;
mod recipes;
mod search;
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
        .merge(canon::router())
        .merge(assets::router())
        .merge(audio::router())
        .merge(collections::router())
        .merge(comments::router())
        .merge(lineage::router())
        .merge(export::router())
        .merge(search::router())
        .merge(context::router())
        .merge(recipes::router())
        .merge(activity::router())
}
