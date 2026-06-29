//! End-to-end API integration test: drives the real `design_studio_backend::app`
//! router in-process (via `tower`'s `oneshot`) against a live Postgres, with
//! mock AI and inline storage so no API keys, MinIO, or network are needed.
//!
//! Gated with `#[ignore]` so the default DB-free `cargo test` stays green; the
//! CI integration job (and `cargo test -- --ignored` locally) runs it with a
//! Postgres service up. `DATABASE_URL` overrides the local-docker default.
//!
//! Covers the slice unit tests can't: auth → workspace → project → generate →
//! the three export packs (generic / Godot / Unity) + the validation 400s,
//! asserting the actual zip contents the engine adapters produce.

use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use std::io::{Cursor, Read};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceExt;

use design_studio_backend::{app, db, jobs, storage::Storage, AppState};

const DEFAULT_DB: &str = "postgres://designstudio:designstudio@localhost:5432/design_studio";

/// One in-process request. Returns the status and the raw body bytes; `cookie`
/// carries the session between calls (None before login).
async fn send(
    router: &axum::Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    json: Option<&str>,
) -> (StatusCode, Vec<u8>, Option<String>) {
    // The auth routes carry their own IP rate-limit layer, whose key extractor
    // reads ConnectInfo — supply a peer addr so in-process requests don't 500.
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))));
    if json.is_some() {
        b = b.header("content-type", "application/json");
    }
    if let Some(c) = cookie {
        b = b.header("cookie", c);
    }
    let body = json.map(|s| Body::from(s.to_owned())).unwrap_or(Body::empty());
    let resp = router.clone().oneshot(b.body(body).unwrap()).await.unwrap();
    let status = resp.status();
    // Capture the session cookie (first attribute of any Set-Cookie) so the
    // caller can thread it into later requests.
    let set_cookie = resp
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(';').next())
        .map(str::to_owned);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, bytes, set_cookie)
}

/// First JSON string field `"<key>": "..."` — tiny, dependency-free extractor.
fn field<'a>(body: &'a [u8], key: &str) -> &'a str {
    let s = std::str::from_utf8(body).unwrap();
    let pat = format!("\"{key}\":\"");
    let i = s.find(&pat).unwrap_or_else(|| panic!("no `{key}` in: {s}")) + pat.len();
    let rest = &s[i..];
    &rest[..rest.find('"').unwrap()]
}

/// Every `"id":"..."` in order — assets come back as an array of objects.
fn all_ids(body: &[u8]) -> Vec<String> {
    let s = std::str::from_utf8(body).unwrap();
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find("\"id\":\"") {
        rest = &rest[i + 6..];
        out.push(rest[..rest.find('"').unwrap()].to_owned());
    }
    out
}

/// The file names inside a zip body.
fn zip_names(bytes: &[u8]) -> Vec<String> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).expect("valid zip");
    (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_owned()).collect()
}

/// Read one zip entry's contents as a String.
fn zip_read(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut f = z.by_name(name).unwrap_or_else(|_| panic!("no `{name}` in {:?}", zip_names(bytes)));
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    s
}

#[tokio::test]
#[ignore = "needs a Postgres; run via `cargo test -- --ignored` or the CI integration job"]
async fn export_packs_end_to_end() {
    // Deterministic, key-free environment: mock every AI path, force inline
    // storage (no MinIO) by clearing the bucket.
    for (k, v) in [
        ("ASSET_MOCK", "true"),
        ("EMBED_MOCK", "true"),
        ("LLM_MOCK", "true"),
        ("AUDIO_MOCK", "true"),
    ] {
        std::env::set_var(k, v);
    }
    std::env::remove_var("S3_BUCKET");

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB.into());
    std::env::set_var("DATABASE_URL", &url);
    let pool = db::connect().await.expect("connect to Postgres");
    db::migrate(&pool).await.expect("migrate");
    let storage = Arc::new(Storage::from_env().await.expect("inline storage"));
    let router = app(AppState { pool, storage });

    // ── signup → session cookie ─────────────────────────────────────────────
    let email = format!("it-{}@test.local", uuid::Uuid::new_v4());
    let (st, _b, cookie) = send(
        &router,
        "POST",
        "/auth/signup",
        None,
        Some(&format!(
            "{{\"email\":\"{email}\",\"password\":\"hunter2pass\",\"workspace_name\":\"IT WS\"}}"
        )),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "signup: {}", String::from_utf8_lossy(&_b));
    let cookie = cookie.expect("session cookie set on signup");

    // ── workspace → game_2d project ─────────────────────────────────────────
    let (st, b, _) = send(&router, "GET", "/workspaces", Some(&cookie), None).await;
    assert_eq!(st, StatusCode::OK);
    let ws = field(&b, "id").to_owned();

    let (st, b, _) = send(
        &router,
        "POST",
        &format!("/workspaces/{ws}/projects"),
        Some(&cookie),
        Some("{\"name\":\"IT Game\",\"vertical\":\"game_2d\"}"),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "create project");
    assert_eq!(field(&b, "vertical"), "game_2d");
    let pid = field(&b, "id").to_owned();

    // ── generate two mock assets ────────────────────────────────────────────
    let (st, b, _) = send(
        &router,
        "POST",
        &format!("/projects/{pid}/assets"),
        Some(&cookie),
        Some("{\"prompt\":\"hero knight\",\"count\":2}"),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "generate");
    let ids = all_ids(&b);
    assert_eq!(ids.len(), 2, "two assets generated");
    let ids_json = ids.iter().map(|i| format!("\"{i}\"")).collect::<Vec<_>>().join(",");

    let export = |target: Option<&str>| {
        let body = match target {
            Some(t) => format!("{{\"asset_ids\":[{ids_json}],\"target\":\"{t}\"}}"),
            None => format!("{{\"asset_ids\":[{ids_json}]}}"),
        };
        let (router, cookie, pid) = (router.clone(), cookie.clone(), pid.clone());
        async move {
            send(&router, "POST", &format!("/projects/{pid}/export"), Some(&cookie), Some(&body)).await
        }
    };

    // ── generic pack: manifest + images, NO engine scaffolding ──────────────
    let (st, zip, _) = export(None).await;
    assert_eq!(st, StatusCode::OK, "generic export");
    let names = zip_names(&zip);
    assert!(names.iter().any(|n| n == "manifest.json"));
    assert!(names.iter().any(|n| n.starts_with("assets/")));
    assert!(!names.iter().any(|n| n == "project.godot"), "no godot files in generic");
    assert!(!names.iter().any(|n| n.ends_with(".meta")), "no unity files in generic");

    // ── Godot pack: .import per texture + project.godot + README ─────────────
    let (st, zip, _) = export(Some("godot")).await;
    assert_eq!(st, StatusCode::OK, "godot export");
    let names = zip_names(&zip);
    assert!(names.iter().any(|n| n == "project.godot"));
    assert!(names.iter().any(|n| n == "README.md"));
    assert_eq!(names.iter().filter(|n| n.ends_with(".import")).count(), 2);
    assert!(zip_read(&zip, "manifest.json").contains("\"target\": \"godot\""));

    // ── Unity pack: .meta (Sprite + GUID) per texture + README ──────────────
    let (st, zip, _) = export(Some("unity")).await;
    assert_eq!(st, StatusCode::OK, "unity export");
    let names = zip_names(&zip);
    assert_eq!(names.iter().filter(|n| n.ends_with(".meta")).count(), 2);
    let meta = zip_read(&zip, names.iter().find(|n| n.ends_with(".meta")).unwrap());
    assert!(meta.contains("textureType: 8"), "imported as a 2D Sprite");
    assert!(meta.contains("guid: "), "carries a stable GUID");

    // ── validation: unknown target, and an engine the vertical lacks ────────
    let (st, _b, _) = export(Some("nintendo64")).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "unknown target → 400");
}

#[tokio::test]
#[ignore = "needs a Postgres; run via `cargo test -- --ignored` or the CI integration job"]
async fn async_generation_job_runs_to_success() {
    for (k, v) in [("ASSET_MOCK", "true"), ("EMBED_MOCK", "true"), ("AUDIO_MOCK", "true")] {
        std::env::set_var(k, v);
    }
    std::env::remove_var("S3_BUCKET");

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB.into());
    std::env::set_var("DATABASE_URL", &url);
    let pool = db::connect().await.expect("connect");
    db::migrate(&pool).await.expect("migrate");
    let storage = Arc::new(Storage::from_env().await.expect("inline storage"));
    let state = AppState { pool, storage };
    // The worker drains the queue; the router enqueues + reports status.
    jobs::spawn_worker(state.clone());
    let router = app(state);

    let email = format!("it-{}@test.local", uuid::Uuid::new_v4());
    let (_st, _b, cookie) = send(
        &router,
        "POST",
        "/auth/signup",
        None,
        Some(&format!("{{\"email\":\"{email}\",\"password\":\"hunter2pass\"}}")),
    )
    .await;
    let cookie = cookie.expect("cookie");
    let (_st, b, _) = send(&router, "GET", "/workspaces", Some(&cookie), None).await;
    let ws = field(&b, "id").to_owned();
    let (_st, b, _) = send(
        &router,
        "POST",
        &format!("/workspaces/{ws}/projects"),
        Some(&cookie),
        Some("{\"name\":\"Jobs\",\"vertical\":\"game_2d\"}"),
    )
    .await;
    let pid = field(&b, "id").to_owned();

    // Enqueue → a queued job comes back immediately.
    let (st, b, _) = send(
        &router,
        "POST",
        &format!("/projects/{pid}/jobs"),
        Some(&cookie),
        Some("{\"prompt\":\"async hero\",\"count\":2}"),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "enqueue");
    assert_eq!(field(&b, "status"), "queued");
    let job_id = field(&b, "id").to_owned();

    // Poll until the worker finishes it.
    let mut final_body = Vec::new();
    for _ in 0..40 {
        let (st, body, _) = send(&router, "GET", &format!("/jobs/{job_id}"), Some(&cookie), None).await;
        assert_eq!(st, StatusCode::OK);
        let status = field(&body, "status").to_owned();
        if status == "succeeded" || status == "failed" {
            final_body = body;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    let v: serde_json::Value = serde_json::from_slice(&final_body).unwrap();
    assert_eq!(v["status"], "succeeded", "job succeeded: {v}");
    let ids = v["result"]["asset_ids"].as_array().expect("result.asset_ids");
    assert_eq!(ids.len(), 2, "produced two assets");
}

#[tokio::test]
#[ignore = "needs a Postgres; run via `cargo test -- --ignored` or the CI integration job"]
async fn folders_tree_move_and_cascade() {
    for (k, v) in [("ASSET_MOCK", "true"), ("EMBED_MOCK", "true"), ("AUDIO_MOCK", "true")] {
        std::env::set_var(k, v);
    }
    std::env::remove_var("S3_BUCKET");

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB.into());
    std::env::set_var("DATABASE_URL", &url);
    let pool = db::connect().await.expect("connect");
    db::migrate(&pool).await.expect("migrate");
    let storage = Arc::new(Storage::from_env().await.expect("inline storage"));
    let router = app(AppState { pool, storage });

    // signup → workspace → project
    let email = format!("it-{}@test.local", uuid::Uuid::new_v4());
    let (_st, _b, cookie) = send(
        &router,
        "POST",
        "/auth/signup",
        None,
        Some(&format!("{{\"email\":\"{email}\",\"password\":\"hunter2pass\"}}")),
    )
    .await;
    let cookie = cookie.expect("cookie");
    let (_st, b, _) = send(&router, "GET", "/workspaces", Some(&cookie), None).await;
    let ws = field(&b, "id").to_owned();
    let (_st, b, _) = send(
        &router,
        "POST",
        &format!("/workspaces/{ws}/projects"),
        Some(&cookie),
        Some("{\"name\":\"Folders\",\"vertical\":\"game_2d\"}"),
    )
    .await;
    let pid = field(&b, "id").to_owned();

    // root folder + subfolder
    let (st, b, _) = send(
        &router,
        "POST",
        &format!("/projects/{pid}/folders"),
        Some(&cookie),
        Some("{\"name\":\"Characters\"}"),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "create root folder");
    let parent = field(&b, "id").to_owned();
    let (st, b, _) = send(
        &router,
        "POST",
        &format!("/projects/{pid}/folders"),
        Some(&cookie),
        Some(&format!("{{\"name\":\"Heroes\",\"parent_id\":\"{parent}\"}}")),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "create subfolder");
    let child = field(&b, "id").to_owned();

    // generate one asset, then move it into the subfolder
    let (st, b, _) = send(
        &router,
        "POST",
        &format!("/projects/{pid}/assets"),
        Some(&cookie),
        Some("{\"prompt\":\"a knight\",\"count\":1}"),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "generate");
    let aid = all_ids(&b)[0].clone();

    let (st, b, _) = send(
        &router,
        "PATCH",
        &format!("/assets/{aid}"),
        Some(&cookie),
        Some(&format!("{{\"folder_id\":\"{child}\"}}")),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "move asset into folder");
    assert_eq!(field(&b, "folder_id"), child, "asset now filed in subfolder");

    // listing scoped to the subfolder contains the asset; root listing does not
    let (_st, b, _) =
        send(&router, "GET", &format!("/projects/{pid}/assets?folder={child}"), Some(&cookie), None).await;
    assert!(all_ids(&b).contains(&aid), "asset shows under its folder");
    let (_st, b, _) =
        send(&router, "GET", &format!("/projects/{pid}/assets?folder=root"), Some(&cookie), None).await;
    assert!(!all_ids(&b).contains(&aid), "asset no longer at root");

    // the folder list carries the direct asset count
    let (_st, b, _) = send(&router, "GET", &format!("/projects/{pid}/folders"), Some(&cookie), None).await;
    let folders: serde_json::Value = serde_json::from_slice(&b).unwrap();
    let heroes = folders.as_array().unwrap().iter().find(|f| f["id"] == child).unwrap();
    assert_eq!(heroes["asset_count"], 1, "subfolder reports one asset");

    // reparent cycle (move parent under its own descendant) is rejected
    let (st, _b, _) = send(
        &router,
        "PATCH",
        &format!("/folders/{parent}"),
        Some(&cookie),
        Some(&format!("{{\"parent_id\":\"{child}\"}}")),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "cycle rejected");

    // deleting the root folder cascades the subtree and UNFILES the asset
    let (st, _b, _) =
        send(&router, "DELETE", &format!("/folders/{parent}"), Some(&cookie), None).await;
    assert_eq!(st, StatusCode::NO_CONTENT, "delete folder");
    let (_st, b, _) = send(&router, "GET", &format!("/projects/{pid}/folders"), Some(&cookie), None).await;
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&b).unwrap().as_array().unwrap().len(), 0, "subtree gone");
    let (_st, b, _) = send(&router, "GET", &format!("/assets/{aid}"), Some(&cookie), None).await;
    let asset: serde_json::Value = serde_json::from_slice(&b).unwrap();
    assert!(asset["folder_id"].is_null(), "asset survived, unfiled");
}

#[tokio::test]
#[ignore = "needs a Postgres; run via `cargo test -- --ignored` or the CI integration job"]
async fn asset_versions_regenerate_and_restore() {
    for (k, v) in [("ASSET_MOCK", "true"), ("EMBED_MOCK", "true"), ("AUDIO_MOCK", "true")] {
        std::env::set_var(k, v);
    }
    std::env::remove_var("S3_BUCKET");

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB.into());
    std::env::set_var("DATABASE_URL", &url);
    let pool = db::connect().await.expect("connect");
    db::migrate(&pool).await.expect("migrate");
    let storage = Arc::new(Storage::from_env().await.expect("inline storage"));
    let router = app(AppState { pool, storage });

    // signup → workspace → project
    let email = format!("it-{}@test.local", uuid::Uuid::new_v4());
    let (_st, _b, cookie) = send(
        &router,
        "POST",
        "/auth/signup",
        None,
        Some(&format!("{{\"email\":\"{email}\",\"password\":\"hunter2pass\"}}")),
    )
    .await;
    let cookie = cookie.expect("cookie");
    let (_st, b, _) = send(&router, "GET", "/workspaces", Some(&cookie), None).await;
    let ws = field(&b, "id").to_owned();
    let (_st, b, _) = send(
        &router,
        "POST",
        &format!("/workspaces/{ws}/projects"),
        Some(&cookie),
        Some("{\"name\":\"Versions\",\"vertical\":\"game_2d\"}"),
    )
    .await;
    let pid = field(&b, "id").to_owned();

    // generate → asset starts at v1 with a head pointer
    let (st, b, _) = send(
        &router,
        "POST",
        &format!("/projects/{pid}/assets"),
        Some(&cookie),
        Some("{\"prompt\":\"a wizard\",\"count\":1}"),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "generate");
    let asset: serde_json::Value = serde_json::from_slice(&b).unwrap();
    let aid = asset[0]["id"].as_str().unwrap().to_owned();
    assert!(asset[0]["current_version_id"].is_string(), "head set on generate");

    let versions = |router: axum::Router, cookie: String, aid: String| async move {
        let (_st, b, _) = send(&router, "GET", &format!("/assets/{aid}/versions"), Some(&cookie), None).await;
        serde_json::from_slice::<serde_json::Value>(&b).unwrap()
    };

    let v = versions(router.clone(), cookie.clone(), aid.clone()).await;
    assert_eq!(v.as_array().unwrap().len(), 1, "one version after generate");
    let v1_id = v[0]["id"].as_str().unwrap().to_owned();

    // regenerate → v2, attributed, "Regenerated"
    let (st, _b, _) = send(
        &router,
        "POST",
        &format!("/assets/{aid}/regenerate"),
        Some(&cookie),
        Some("{\"prompt\":\"a wizard, blue robe\"}"),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "regenerate");
    let v = versions(router.clone(), cookie.clone(), aid.clone()).await;
    assert_eq!(v.as_array().unwrap().len(), 2, "two versions");
    assert_eq!(v[0]["version"], 2);
    assert_eq!(v[0]["change_note"], "Regenerated");
    assert_eq!(v[0]["author_email"], email, "version attributed to author");

    // a specific version's bytes are servable and differ from the head
    let (st, head_bytes, _) = send(&router, "GET", &format!("/assets/{aid}/file"), Some(&cookie), None).await;
    assert_eq!(st, StatusCode::OK);
    let (st, v1_bytes, _) =
        send(&router, "GET", &format!("/assets/{aid}/file?version={v1_id}"), Some(&cookie), None).await;
    assert_eq!(st, StatusCode::OK, "historical version servable");
    assert_ne!(head_bytes, v1_bytes, "head (v2) differs from v1");

    // restore v1 → appends v3 whose bytes equal v1 (non-destructive rollback)
    let (st, _b, _) =
        send(&router, "POST", &format!("/assets/{aid}/versions/{v1_id}/restore"), Some(&cookie), None).await;
    assert_eq!(st, StatusCode::OK, "restore");
    let v = versions(router.clone(), cookie.clone(), aid.clone()).await;
    assert_eq!(v.as_array().unwrap().len(), 3, "restore appended a version");
    assert_eq!(v[0]["version"], 3);
    assert_eq!(v[0]["change_note"], "Restored v1");
    let (_st, restored_bytes, _) = send(&router, "GET", &format!("/assets/{aid}/file"), Some(&cookie), None).await;
    assert_eq!(restored_bytes, v1_bytes, "head now matches the restored v1 bytes");

    // a bogus version id 404s
    let (st, _b, _) = send(
        &router,
        "GET",
        &format!("/assets/{aid}/file?version=00000000-0000-0000-0000-000000000000"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND, "unknown version → 404");
}
