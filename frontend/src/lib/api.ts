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

// ── Canon (versioned style rules + exemplars) ──────────────────────────────────
export interface Canon {
  id: string
  project_id: string
  parent_id: string | null
  version: number
  data: unknown // { style: {...}, negative: [...], exemplar_asset_ids: [...] }
  created_at: string
}

/** Current canon, or null if none defined yet (backend 404). */
export const getCanon = (projectId: string) =>
  request<Canon>(`/projects/${projectId}/canon`).catch((e) => {
    if (e instanceof ApiError && e.status === 404) return null
    throw e
  })

/** Append a new canon version (auto-incremented + lineage on the backend). */
export const saveCanon = (projectId: string, data: unknown) =>
  request<Canon>(`/projects/${projectId}/canon`, {
    method: 'POST',
    body: JSON.stringify({ data }),
  })

// ── Assets ────────────────────────────────────────────────────────────────────
export type AssetStatus = 'candidate' | 'approved' | 'rejected' | 'needs_review'

export interface Asset {
  id: string
  project_id: string
  kind: string
  s3_key: string // object-storage key (or data/http URL in inline mode)
  url: string // stable, browser-usable image URL — use for <img src> / props.src
  mime_type: string | null
  prompt: string | null
  role: string | null
  status: AssetStatus
  tags: string[]
  source_kind: string // 'uploaded' | 'seeded' | 'derived'
  derivation: string | null // for derivatives: the preset/instruction used
  canon_version_id: string | null
  created_at: string
}

/** An asset plus its lineage — the base it came from + its derivatives. */
export interface AssetDetail extends Asset {
  base: Asset | null
  derivatives: Asset[]
}

export const listAssets = (projectId: string) =>
  request<Asset[]>(`/projects/${projectId}/assets`)

export const generateAssets = (projectId: string, prompt: string, count = 1) =>
  request<Asset[]>(`/projects/${projectId}/assets`, {
    method: 'POST',
    body: JSON.stringify({ prompt, count }),
  })

/** Derive variants from a base asset, conditioned on the base + current canon. */
export const deriveAssets = (
  projectId: string,
  baseId: string,
  instruction: string,
  count = 1,
) =>
  request<Asset[]>(`/projects/${projectId}/assets/${baseId}/derive`, {
    method: 'POST',
    body: JSON.stringify({ instruction, count }),
  })

/** Approve / reject / flag a candidate (the review gate). */
export const setAssetStatus = (assetId: string, status: AssetStatus) =>
  request<Asset>(`/assets/${assetId}`, {
    method: 'PATCH',
    body: JSON.stringify({ status }),
  })

/** One asset with its lineage (base + derivatives). */
export const getAsset = (id: string) => request<AssetDetail>(`/assets/${id}`)

/** Patch editable metadata (role / tags). Only provided fields change. */
export const updateAsset = (id: string, patch: { role?: string; tags?: string[] }) =>
  request<Asset>(`/assets/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(patch),
  })

/** Upload a base/reference image. Raw bytes body, not multipart. */
export const uploadAsset = async (
  projectId: string,
  file: File,
  role?: string,
): Promise<Asset> => {
  const q = role ? `?role=${encodeURIComponent(role)}` : ''
  const res = await fetch(`${BASE}/projects/${projectId}/assets/upload${q}`, {
    method: 'POST',
    credentials: 'include',
    headers: { 'content-type': file.type || 'image/png' },
    body: file,
  })
  const body = await res.json().catch(() => null)
  if (!res.ok) {
    const msg =
      (body && typeof body === 'object' && 'error' in body && String(body.error)) ||
      `upload failed (${res.status})`
    throw new ApiError(res.status, msg)
  }
  return body as Asset
}
