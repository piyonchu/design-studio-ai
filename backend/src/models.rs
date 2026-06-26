use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

// ── Postgres enum mappings ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "workspace_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    // Ordered viewer < editor < owner so role comparisons gate write access.
    Viewer,
    Editor,
    Owner,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "asset_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Image,
    Icon,
    Illustration,
    Audio,
    Svg,
}

/// Review lifecycle: everything starts `Candidate`; only `Approved` enters the
/// canon and influences future derivations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "asset_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AssetStatus {
    Candidate,
    Approved,
    Rejected,
    NeedsReview,
}

// ── Row structs (DB → JSON responses) ────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Project {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub brief: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Request DTOs ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateWorkspace {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    pub name: String,
    #[serde(default)]
    pub brief: Option<String>,
}

// ── Canon (versioned style rules + exemplars) ─────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct Canon {
    pub id: Uuid,
    pub project_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub version: i32,
    pub data: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCanon {
    /// Free-form: { style: {...}, negative: [...], exemplar_asset_ids: [...] }.
    pub data: Value,
}

// ── Auth ──────────────────────────────────────────────────────────────────────

/// Public user representation — never includes the password hash.
#[derive(Debug, Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

/// Internal row for credential verification (carries the hash).
#[derive(Debug, FromRow)]
pub struct UserCredentials {
    pub id: Uuid,
    pub password_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub workspace_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Signup response: the new user plus the default workspace created for them.
#[derive(Debug, Serialize)]
pub struct SignupResponse {
    pub user: User,
    pub workspace: Workspace,
}

// ── Assets ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct Asset {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: AssetKind,
    /// Object-storage key (S3/MinIO), or a `data:`/`http` URL in inline mode.
    pub s3_key: String,
    pub mime_type: Option<String>,
    pub prompt: Option<String>,
    pub role: Option<String>,
    pub status: AssetStatus,
    pub tags: Vec<String>,
    /// How the asset entered the library: 'uploaded' | 'seeded' | 'derived'.
    pub source_kind: String,
    /// For derivatives: the preset/instruction used, and the canon version it
    /// was produced under (null for uploaded/seeded assets).
    pub derivation: Option<String>,
    pub canon_version_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    /// Stable, browser-usable URL for the image. Not stored — filled in by the
    /// route after fetching (see `routes::assets`). For object-stored assets
    /// this points at `GET /assets/:id/file`; inline assets expose the URL
    /// directly.
    #[sqlx(default)]
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateAssets {
    pub prompt: String,
    #[serde(default)]
    pub count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct DeriveAssets {
    pub instruction: String,
    #[serde(default)]
    pub count: Option<u32>,
}

/// Partial update — only the provided fields change (COALESCE on the backend).
#[derive(Debug, Deserialize)]
pub struct UpdateAsset {
    #[serde(default)]
    pub status: Option<AssetStatus>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// An asset plus its lineage: the base it was derived from, and its derivatives.
#[derive(Debug, Serialize)]
pub struct AssetDetail {
    #[serde(flatten)]
    pub asset: Asset,
    pub base: Option<Asset>,
    pub derivatives: Vec<Asset>,
}

// ── Collections ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct Collection {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub cover_asset_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// List row: a collection with its item count + a cover asset to thumbnail.
#[derive(Debug, Serialize, FromRow)]
pub struct CollectionSummary {
    pub id: Uuid,
    pub name: String,
    pub item_count: i64,
    pub cover_asset_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// A collection plus its assets.
#[derive(Debug, Serialize)]
pub struct CollectionDetail {
    #[serde(flatten)]
    pub collection: Collection,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCollection {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddItems {
    pub asset_ids: Vec<Uuid>,
}

// ── Comments (collaboration) ─────────────────────────────────────────────────

/// A comment on an asset, with the author's email joined for display. The
/// author is nullable: a deleted account leaves the discussion intact.
#[derive(Debug, Serialize, FromRow)]
pub struct AssetComment {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub author_id: Option<Uuid>,
    pub author_email: Option<String>,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateComment {
    pub body: String,
}

// ── Lineage + canon propagation ──────────────────────────────────────────────

/// A directed edge in the asset graph (currently always `derived_from`).
#[derive(Debug, Serialize, FromRow)]
pub struct AssetLink {
    pub from_asset: Uuid,
    pub to_asset: Uuid,
    pub relation: String,
}

/// The whole project graph in one payload: every asset (the nodes) + the
/// derivation edges. The frontend lays out roots → derivatives from this.
#[derive(Debug, Serialize)]
pub struct LineageGraph {
    pub assets: Vec<Asset>,
    pub links: Vec<AssetLink>,
}

#[derive(Debug, Deserialize)]
pub struct ReconcileRequest {
    pub asset_ids: Vec<Uuid>,
}

// ── Export ───────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub asset_ids: Vec<Uuid>,
}

/// One asset's pre-export verdict: the filename it would get in the pack, its
/// decoded raster facts, and any issues a deterministic check surfaced.
#[derive(Debug, Serialize)]
pub struct AssetCheck {
    pub id: Uuid,
    pub filename: String,
    pub role: Option<String>,
    /// Grouping key (slugged role, or "ungrouped"). Drives the zip folder + the
    /// manifest groups so an engine adapter can map groups → animations later.
    pub group: String,
    pub tags: Vec<String>,
    pub status: AssetStatus,
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_alpha: Option<bool>,
    pub issues: Vec<String>,
    /// True when nothing blocking was found (issues may still carry warnings).
    pub ok: bool,
}

#[derive(Debug, Serialize)]
pub struct ExportReport {
    pub assets: Vec<AssetCheck>,
    pub ok_count: usize,
    pub issue_count: usize,
}
