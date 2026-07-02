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

#[tokio::test]
#[ignore = "needs a Postgres; run via `cargo test -- --ignored` or the CI integration job"]
async fn deterministic_edit_appends_version() {
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
        Some("{\"name\":\"Edits\",\"vertical\":\"game_2d\"}"),
    )
    .await;
    let pid = field(&b, "id").to_owned();

    // Upload a real PNG so the edit has actual raster bytes to transform.
    let png = {
        use std::io::Cursor;
        let img = image::RgbaImage::from_pixel(10, 6, image::Rgba([180, 40, 40, 255]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    };
    let req = Request::builder()
        .method("POST")
        .uri(format!("/projects/{pid}/assets/upload"))
        .header("content-type", "image/png")
        .header("cookie", &cookie)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))))
        .body(Body::from(png))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "upload base");
    let b = to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    let aid = field(&b, "id").to_owned();

    // Rotate 90° → a new version; the head image is now 6×10.
    let (st, _b, _) = send(
        &router,
        "POST",
        &format!("/assets/{aid}/edit"),
        Some(&cookie),
        Some("{\"op\":\"rotate\",\"degrees\":90}"),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "edit rotate");

    let (_st, b, _) = send(&router, "GET", &format!("/assets/{aid}/versions"), Some(&cookie), None).await;
    let versions: serde_json::Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(versions.as_array().unwrap().len(), 2, "edit appended a version");
    assert_eq!(versions[0]["change_note"], "Rotated 90°");

    let (_st, head, _) = send(&router, "GET", &format!("/assets/{aid}/file"), Some(&cookie), None).await;
    let img = image::load_from_memory(&head).expect("valid png");
    assert_eq!((img.width(), img.height()), (6, 10), "rotated dimensions");

    // An invalid op is rejected without touching history.
    let (st, _b, _) = send(
        &router,
        "POST",
        &format!("/assets/{aid}/edit"),
        Some(&cookie),
        Some("{\"op\":\"rotate\",\"degrees\":45}"),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "bad rotation rejected");
}

#[tokio::test]
#[ignore = "needs a Postgres; run via `cargo test -- --ignored` or the CI integration job"]
async fn reviewer_gate_blocks_editor_approval() {
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

    // Owner A: workspace + project + a generated asset.
    let a_email = format!("owner-{}@test.local", uuid::Uuid::new_v4());
    let (_st, _b, cookie_a) = send(
        &router,
        "POST",
        "/auth/signup",
        None,
        Some(&format!("{{\"email\":\"{a_email}\",\"password\":\"hunter2pass\"}}")),
    )
    .await;
    let cookie_a = cookie_a.expect("A cookie");
    let (_st, b, _) = send(&router, "GET", "/workspaces", Some(&cookie_a), None).await;
    let ws = field(&b, "id").to_owned();
    let (_st, b, _) = send(
        &router,
        "POST",
        &format!("/workspaces/{ws}/projects"),
        Some(&cookie_a),
        Some("{\"name\":\"Perms\",\"vertical\":\"game_2d\"}"),
    )
    .await;
    let pid = field(&b, "id").to_owned();
    let (_st, b, _) = send(
        &router,
        "POST",
        &format!("/projects/{pid}/assets"),
        Some(&cookie_a),
        Some("{\"prompt\":\"a knight\",\"count\":1}"),
    )
    .await;
    let aid = all_ids(&b)[0].clone();

    // Editor B: separate signup, then invited into A's workspace as editor.
    let b_email = format!("editor-{}@test.local", uuid::Uuid::new_v4());
    let (_st, bbody, cookie_b) = send(
        &router,
        "POST",
        "/auth/signup",
        None,
        Some(&format!("{{\"email\":\"{b_email}\",\"password\":\"hunter2pass\"}}")),
    )
    .await;
    let cookie_b = cookie_b.expect("B cookie");
    let b_id = field(&bbody, "id").to_owned(); // first id = user.id
    let (st, _b, _) = send(
        &router,
        "POST",
        &format!("/workspaces/{ws}/members"),
        Some(&cookie_a),
        Some(&format!("{{\"email\":\"{b_email}\",\"role\":\"editor\"}}")),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "invite B as editor");

    // B (editor) cannot approve, and /access says so.
    let (_st, acc, _) = send(&router, "GET", &format!("/projects/{pid}/access"), Some(&cookie_b), None).await;
    let acc: serde_json::Value = serde_json::from_slice(&acc).unwrap();
    assert_eq!(acc["role"], "editor");
    assert_eq!(acc["can_approve"], false, "editor cannot approve");
    let (st, _b, _) = send(
        &router,
        "PATCH",
        &format!("/assets/{aid}"),
        Some(&cookie_b),
        Some("{\"status\":\"approved\"}"),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "editor approval blocked by review gate");

    // B can still flag for review (editor-level write is allowed).
    let (st, _b, _) = send(
        &router,
        "PATCH",
        &format!("/assets/{aid}"),
        Some(&cookie_b),
        Some("{\"status\":\"needs_review\"}"),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "editor may flag needs_review");

    // Owner A promotes B to reviewer on this project.
    let (st, _b, _) = send(
        &router,
        "PUT",
        &format!("/projects/{pid}/members/{b_id}"),
        Some(&cookie_a),
        Some("{\"role\":\"reviewer\"}"),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT, "owner sets reviewer override");

    // Now B can approve.
    let (_st, acc, _) = send(&router, "GET", &format!("/projects/{pid}/access"), Some(&cookie_b), None).await;
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&acc).unwrap()["can_approve"], true);
    let (st, _b, _) = send(
        &router,
        "PATCH",
        &format!("/assets/{aid}"),
        Some(&cookie_b),
        Some("{\"status\":\"approved\"}"),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "reviewer approval allowed");

    // An editor cannot hand out roles (owner-only).
    let (st, _b, _) = send(
        &router,
        "PUT",
        &format!("/projects/{pid}/members/{b_id}"),
        Some(&cookie_b),
        Some("{\"role\":\"owner\"}"),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "non-owner cannot assign roles");
}

#[tokio::test]
#[ignore = "needs a Postgres; run via `cargo test -- --ignored` or the CI integration job"]
async fn inpaint_mock_changes_masked_region_and_versions() {
    for (k, v) in [("ASSET_MOCK", "true"), ("EMBED_MOCK", "true"), ("EDIT_MOCK", "true")] {
        std::env::set_var(k, v);
    }
    std::env::remove_var("S3_BUCKET");

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB.into());
    std::env::set_var("DATABASE_URL", &url);
    let pool = db::connect().await.expect("connect");
    db::migrate(&pool).await.expect("migrate");
    let storage = Arc::new(Storage::from_env().await.expect("inline storage"));
    let router = app(AppState { pool, storage });

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
        Some("{\"name\":\"Inpaint\",\"vertical\":\"game_2d\"}"),
    )
    .await;
    let pid = field(&b, "id").to_owned();

    // Upload a flat gray base (8×8).
    let base_png = {
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([150, 150, 150, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    };
    let req = Request::builder()
        .method("POST")
        .uri(format!("/projects/{pid}/assets/upload"))
        .header("content-type", "image/png")
        .header("cookie", &cookie)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))))
        .body(Body::from(base_png))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let b = to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    let aid = field(&b, "id").to_owned();

    // Mask: paint the top-left 4×8 region opaque white; the rest transparent.
    let mask_data = {
        let mut m = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 0]));
        for y in 0..8 {
            for x in 0..4 {
                m.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(m).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        use base64::Engine;
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
    };

    let (st, _b, _) = send(
        &router,
        "POST",
        &format!("/assets/{aid}/inpaint"),
        Some(&cookie),
        Some(&format!("{{\"mask\":\"{mask_data}\",\"prompt\":\"a red hat\"}}")),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "inpaint");

    let (_st, b, _) = send(&router, "GET", &format!("/assets/{aid}/versions"), Some(&cookie), None).await;
    let versions: serde_json::Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(versions.as_array().unwrap().len(), 2, "inpaint appended a version");
    assert!(versions[0]["change_note"].as_str().unwrap().starts_with("Inpainted:"));

    // Head image: masked region changed, unmasked region unchanged.
    let (_st, head, _) = send(&router, "GET", &format!("/assets/{aid}/file"), Some(&cookie), None).await;
    let img = image::load_from_memory(&head).unwrap().to_rgba8();
    assert_ne!(img.get_pixel(0, 0).0, [150, 150, 150, 255], "masked pixel changed");
    assert_eq!(img.get_pixel(7, 0).0, [150, 150, 150, 255], "unmasked pixel unchanged");
}

#[tokio::test]
#[ignore = "needs a Postgres; run via `cargo test -- --ignored` or the CI integration job"]
async fn generation_conditions_on_an_approved_exemplar() {
    for (k, v) in [("ASSET_MOCK", "true"), ("EMBED_MOCK", "true")] {
        std::env::set_var(k, v);
    }
    std::env::remove_var("S3_BUCKET");

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB.into());
    std::env::set_var("DATABASE_URL", &url);
    let pool = db::connect().await.expect("connect");
    db::migrate(&pool).await.expect("migrate");
    let storage = Arc::new(Storage::from_env().await.expect("inline storage"));
    let router = app(AppState { pool: pool.clone(), storage });

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
        Some("{\"name\":\"Exemplar\",\"vertical\":\"game_2d\"}"),
    )
    .await;
    let pid = field(&b, "id").to_owned();

    // Two raster PNG exemplars, each approved + marked exemplar.
    let mut exemplar_ids = Vec::new();
    for color in [[200u8, 40, 40, 255], [40, 40, 200, 255]] {
        let png = {
            let img = image::RgbaImage::from_pixel(8, 8, image::Rgba(color));
            let mut buf = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(img).write_to(&mut buf, image::ImageFormat::Png).unwrap();
            buf.into_inner()
        };
        let req = Request::builder()
            .method("POST")
            .uri(format!("/projects/{pid}/assets/upload"))
            .header("content-type", "image/png")
            .header("cookie", &cookie)
            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))))
            .body(Body::from(png))
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        let b = to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
        let id = field(&b, "id").to_owned();
        let (st, _b, _) = send(
            &router,
            "PATCH",
            &format!("/assets/{id}"),
            Some(&cookie),
            Some("{\"status\":\"approved\",\"exemplar\":true}"),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "approve + mark exemplar");
        exemplar_ids.push(id);
    }

    // Generate → run_generate selects an exemplar and conditions on it,
    // recording metadata.exemplar_id.
    let (st, b, _) = send(
        &router,
        "POST",
        &format!("/projects/{pid}/assets"),
        Some(&cookie),
        Some("{\"prompt\":\"a blue knight\",\"count\":1}"),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "generate");
    let new_id = all_ids(&b)[0].clone();

    // The new asset must be conditioned on one of the two approved exemplars
    // (the smartest-exemplar query ran and returned a valid candidate).
    let chosen: Option<String> =
        sqlx::query_scalar("SELECT metadata->>'exemplar_id' FROM assets WHERE id = $1::uuid")
            .bind(&new_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let chosen = chosen.expect("generation recorded an exemplar_id");
    assert!(exemplar_ids.contains(&chosen), "conditioned on an approved exemplar");
}

/// B3 (`POST /assets/:id/versions`, client-rendered bytes) + the embedding QA
/// gate (`style_fit` scoring at creation + the `?off_style` board filter).
#[tokio::test]
#[ignore = "needs a Postgres; run via `cargo test -- --ignored` or the CI integration job"]
async fn save_version_and_style_qa_gate() {
    for (k, v) in [("ASSET_MOCK", "true"), ("EMBED_MOCK", "true")] {
        std::env::set_var(k, v);
    }
    std::env::remove_var("S3_BUCKET");

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB.into());
    std::env::set_var("DATABASE_URL", &url);
    let pool = db::connect().await.expect("connect");
    db::migrate(&pool).await.expect("migrate");
    let storage = Arc::new(Storage::from_env().await.expect("inline storage"));
    let router = app(AppState { pool, storage });

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
        Some("{\"name\":\"QA\",\"vertical\":\"game_2d\"}"),
    )
    .await;
    let pid = field(&b, "id").to_owned();

    let png = |color: [u8; 4]| {
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba(color));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    };
    let upload = |bytes: Vec<u8>| {
        let (router, cookie, pid) = (router.clone(), cookie.clone(), pid.clone());
        async move {
            let req = Request::builder()
                .method("POST")
                .uri(format!("/projects/{pid}/assets/upload"))
                .header("content-type", "image/png")
                .header("cookie", &cookie)
                .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))))
                .body(Body::from(bytes))
                .unwrap();
            let resp = router.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED, "upload");
            let b = to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
            field(&b, "id").to_owned()
        }
    };

    // Approve one uploaded asset so later creations have a peer to score against.
    let approved = upload(png([200, 40, 40, 255])).await;
    let (st, _b, _) = send(
        &router,
        "PATCH",
        &format!("/assets/{approved}"),
        Some(&cookie),
        Some("{\"status\":\"approved\"}"),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "approve peer");

    // ── QA gate: a fresh generation gets a style_fit score vs the approved peer.
    let (st, b, _) = send(
        &router,
        "POST",
        &format!("/projects/{pid}/assets"),
        Some(&cookie),
        Some("{\"prompt\":\"a knight\",\"count\":1}"),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "generate");
    let gen: serde_json::Value = serde_json::from_slice(&b).unwrap();
    let gen_fit = gen[0]["style_fit"].as_f64();
    assert!(gen_fit.is_some(), "generation scored a style_fit vs the approved peer: {gen}");

    // The off-style filter returns only sub-threshold assets (never the peer
    // itself, which has no score, and nothing >= 0.5 by default).
    let (st, b, _) = send(
        &router,
        "GET",
        &format!("/projects/{pid}/assets?off_style=true"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "off_style filter");
    let page: serde_json::Value = serde_json::from_slice(&b).unwrap();
    for item in page["items"].as_array().unwrap() {
        let fit = item["style_fit"].as_f64().expect("filtered assets carry a score");
        assert!(fit < 0.5, "off_style returns only sub-threshold assets (got {fit})");
    }

    // ── B3: store client-rendered bytes as a new version.
    let base = upload(png([40, 40, 200, 255])).await;
    let painted = png([10, 200, 10, 255]);
    let req = Request::builder()
        .method("POST")
        .uri(format!("/assets/{base}/versions?note=Hand-painted%20test"))
        .header("content-type", "image/png")
        .header("cookie", &cookie)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))))
        .body(Body::from(painted.clone()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "save painted version");

    let (_st, b, _) = send(&router, "GET", &format!("/assets/{base}/versions"), Some(&cookie), None).await;
    let versions: serde_json::Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(versions.as_array().unwrap().len(), 2, "paint appended a version");
    assert_eq!(versions[0]["change_note"], "Hand-painted test");

    // The head now serves exactly the painted bytes.
    let (_st, head, _) = send(&router, "GET", &format!("/assets/{base}/file"), Some(&cookie), None).await;
    assert_eq!(head, painted, "head bytes are the client-rendered image");

    // Garbage bytes are rejected without touching history.
    let req = Request::builder()
        .method("POST")
        .uri(format!("/assets/{base}/versions"))
        .header("content-type", "image/png")
        .header("cookie", &cookie)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))))
        .body(Body::from(vec![1u8, 2, 3, 4]))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "undecodable bytes rejected");
    let (_st, b, _) = send(&router, "GET", &format!("/assets/{base}/versions"), Some(&cookie), None).await;
    let versions: serde_json::Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(versions.as_array().unwrap().len(), 2, "failed save left history untouched");
}
