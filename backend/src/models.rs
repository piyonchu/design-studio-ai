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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "link_relation", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum LinkRelation {
    DerivedFrom,
    References,
    Contains,
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
