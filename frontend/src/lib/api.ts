// Thin fetch wrapper over the Rust backend. Same-origin via the Vite `/api`
// proxy, so the httpOnly ds_session cookie is sent automatically.

const BASE = '/api'

export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    credentials: 'include',
    headers: { 'content-type': 'application/json' },
    ...init,
  })
  if (res.status === 204) return undefined as T
  const body = await res.json().catch(() => null)
  if (!res.ok) {
    const msg =
      (body && typeof body === 'object' && 'error' in body && String(body.error)) ||
      `request failed (${res.status})`
    throw new ApiError(res.status, msg)
  }
  return body as T
}

// ── Types (mirror the backend models) ───────────────────────────────────────
export interface User {
  id: string
  email: string
  created_at: string
}
export interface Workspace {
  id: string
  name: string
  created_at: string
}
export interface Project {
  id: string
  workspace_id: string
  name: string
  brief: string | null
  created_at: string
}

export type ArtifactKind =
  | 'idea'
  | 'user_flow'
  | 'wireframe'
  | 'design_system'
  | 'ui_screen'
export type ChangeSource = 'manual' | 'ai' | 'import'

export interface Artifact {
  id: string
  project_id: string
  kind: ArtifactKind
  name: string
  head_version_id: string | null
  created_at: string
}
export interface ArtifactVersion {
  id: string
  artifact_id: string
  parent_id: string | null
  content: unknown
  change_source: ChangeSource
  change_summary: string | null
  prompt: string | null
  created_at: string
}
export interface ArtifactWithHead extends Artifact {
  head_version: ArtifactVersion | null
}

// ── Auth ─────────────────────────────────────────────────────────────────────
export const signup = (email: string, password: string, workspace_name?: string) =>
  request<{ user: User; workspace: Workspace }>('/auth/signup', {
    method: 'POST',
    body: JSON.stringify({ email, password, workspace_name }),
  })

export const login = (email: string, password: string) =>
  request<User>('/auth/login', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  })

export const logout = () => request<void>('/auth/logout', { method: 'POST' })

export const me = () => request<User>('/auth/me')

// ── Workspaces & projects ─────────────────────────────────────────────────────
export const listWorkspaces = () => request<Workspace[]>('/workspaces')

export const listProjects = (workspaceId: string) =>
  request<Project[]>(`/workspaces/${workspaceId}/projects`)

export const createProject = (workspaceId: string, name: string, brief?: string) =>
  request<Project>(`/workspaces/${workspaceId}/projects`, {
    method: 'POST',
    body: JSON.stringify({ name, brief }),
  })

export const getProject = (id: string) => request<Project>(`/projects/${id}`)

// ── Artifacts ─────────────────────────────────────────────────────────────────
export const listArtifacts = (projectId: string) =>
  request<Artifact[]>(`/projects/${projectId}/artifacts`)

export const getArtifact = (id: string) =>
  request<ArtifactWithHead>(`/artifacts/${id}`)

export const generate = (
  projectId: string,
  body: { kind: ArtifactKind; prompt: string; parent_artifact_id?: string },
) =>
  request<ArtifactWithHead>(`/projects/${projectId}/artifacts/generate`, {
    method: 'POST',
    body: JSON.stringify(body),
  })

export const aiEdit = (artifactId: string, prompt: string) =>
  request<ArtifactVersion>(`/artifacts/${artifactId}/ai-edit`, {
    method: 'POST',
    body: JSON.stringify({ prompt }),
  })

export const addVersion = (
  artifactId: string,
  body: { content: unknown; change_source: ChangeSource; change_summary?: string },
) =>
  request<ArtifactVersion>(`/artifacts/${artifactId}/versions`, {
    method: 'POST',
    body: JSON.stringify(body),
  })

// ── Assets ────────────────────────────────────────────────────────────────────
export interface Asset {
  id: string
  project_id: string
  screen_id: string | null
  kind: string
  s3_key: string // object-storage key (or data/http URL in inline mode)
  url: string // stable, browser-usable image URL — use for <img src> / props.src
  mime_type: string | null
  prompt: string | null
  created_at: string
}

export const listAssets = (projectId: string) =>
  request<Asset[]>(`/projects/${projectId}/assets`)

export const generateAssets = (projectId: string, prompt: string, count = 1) =>
  request<Asset[]>(`/projects/${projectId}/assets`, {
    method: 'POST',
    body: JSON.stringify({ prompt, count }),
  })

export const attachAsset = (assetId: string, screenArtifactId: string) =>
  request<Asset>(`/assets/${assetId}/attach`, {
    method: 'POST',
    body: JSON.stringify({ screen_artifact_id: screenArtifactId }),
  })
