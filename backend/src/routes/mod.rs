mod activity;
mod assets;
mod audio;
mod jobs;
mod auth;
mod canon;
mod collections;
mod comments;
mod context;
mod export;
mod folders;
mod lineage;
mod project_members;
mod projects;
mod recipes;
mod search;
mod usage;
mod workspaces;

use axum::Router;

use crate::AppState;

/// The async-generation core, shared by the sync `generate` route and the job
/// worker (`crate::jobs`). Re-exported so the worker can reach it without the
/// `assets` submodule being public.
pub(crate) use assets::run_generate;

/// All API routes, merged into one router shared with the app's `AppState`.
/// (The stricter auth rate limit is applied inside `auth::router()`.)
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(workspaces::router())
        .merge(projects::router())
        .merge(project_members::router())
        .merge(canon::router())
        .merge(assets::router())
        .merge(audio::router())
        .merge(collections::router())
        .merge(folders::router())
        .merge(comments::router())
        .merge(lineage::router())
        .merge(export::router())
        .merge(search::router())
        .merge(context::router())
        .merge(recipes::router())
        .merge(activity::router())
        .merge(usage::router())
        .merge(jobs::router())
}
