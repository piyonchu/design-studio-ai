use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

// ── Postgres enum mappings ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "artifact_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Idea,
    UserFlow,
    Wireframe,
    DesignSystem,
    UiScreen,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "change_source", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ChangeSource {
    Manual,
    Ai,
    Import,
}

impl Default for ChangeSource {
    fn default() -> Self {
        ChangeSource::Manual
    }
}

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
#[sqlx(type_name = "link_relation", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum LinkRelation {
    DerivedFrom,
    References,
    Contains,
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

impl Default for LinkRelation {
    fn default() -> Self {
        LinkRelation::DerivedFrom
    }
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

#[derive(Debug, Serialize, FromRow)]
pub struct Artifact {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: ArtifactKind,
    pub name: String,
    pub head_version_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ArtifactVersion {
    pub id: Uuid,
    pub artifact_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub content: Value,
    pub change_source: ChangeSource,
    pub change_summary: Option<String>,
    pub prompt: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ArtifactLink {
    pub id: Uuid,
    pub from_artifact_id: Uuid,
    pub to_artifact_id: Uuid,
    pub relation: LinkRelation,
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

#[derive(Debug, Deserialize)]
pub struct CreateArtifact {
    pub kind: ArtifactKind,
    pub name: String,
    pub content: Value,
    #[serde(default)]
    pub change_source: ChangeSource,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVersion {
    pub content: Value,
    #[serde(default)]
    pub change_source: ChangeSource,
    #[serde(default)]
    pub change_summary: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLink {
    pub to_artifact_id: Uuid,
    #[serde(default)]
    pub relation: LinkRelation,
}

/// An artifact plus its current head version — returned by create / get.
#[derive(Debug, Serialize)]
pub struct ArtifactWithHead {
    #[serde(flatten)]
    pub artifact: Artifact,
    pub head_version: Option<ArtifactVersion>,
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
    pub screen_id: Option<Uuid>,
    pub kind: AssetKind,
    pub s3_key: String, // holds the image URL / data URL until real S3 lands
    pub mime_type: Option<String>,
    pub prompt: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateAssets {
    pub prompt: String,
    #[serde(default)]
    pub count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct AttachAsset {
    pub screen_artifact_id: Uuid,
}
