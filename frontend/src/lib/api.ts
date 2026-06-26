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
  vertical: string // 'game_2d' | 'manhwa'
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

export const createProject = (
  workspaceId: string,
  name: string,
  brief?: string,
  vertical?: string,
) =>
  request<Project>(`/workspaces/${workspaceId}/projects`, {
    method: 'POST',
    body: JSON.stringify({ name, brief, vertical }),
  })

export const getProject = (id: string) => request<Project>(`/projects/${id}`)

// ── Canon (versioned style rules + exemplars) ──────────────────────────────────
export interface Canon {
  id: string
  project_id: string
  parent_id: string | null
  version: number
  data: unknown // { style: {...}, negative: [...], exemplar_asset_ids: [...] }
  change_note: string | null // auto-generated diff vs the previous version
  created_at: string
}

/** Full canon version history, newest first (each with its change note). */
export const getCanonHistory = (projectId: string) =>
  request<Canon[]>(`/projects/${projectId}/canon/history`)

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
  name: string | null // explicit display name; null → derive from role/prompt
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
  exemplar: boolean // approved style anchor — conditions future generation
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

/** Generate audio assets (SFX / loops) — kind='audio'. */
export const generateAudio = (projectId: string, prompt: string, count = 1) =>
  request<Asset[]>(`/projects/${projectId}/audio`, {
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

/** Patch editable metadata. Only provided fields change. */
export const updateAsset = (
  id: string,
  patch: { name?: string; role?: string; tags?: string[]; exemplar?: boolean },
) =>
  request<Asset>(`/assets/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(patch),
  })

/** A friendly label: the explicit name, else an auto-derived one (role + prompt). */
export function displayName(a: Asset): string {
  if (a.name?.trim()) return a.name.trim()
  const role = a.role?.trim()
  const text = (a.prompt ?? a.derivation ?? '').trim().replace(/\s+/g, ' ')
  const short = text.split(' ').slice(0, 5).join(' ')
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1)
  if (role && short) return `${cap(role)} · ${short}`
  if (short) return short
  if (role) return cap(role)
  return a.kind === 'audio' ? 'Audio clip' : 'Untitled asset'
}

/** Delete an asset (its lineage edges cascade). */
export const deleteAsset = (id: string) => request<void>(`/assets/${id}`, { method: 'DELETE' })

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

// ── Collections ────────────────────────────────────────────────────────────────
export interface CollectionSummary {
  id: string
  name: string
  item_count: number
  cover_asset_id: string | null
  created_at: string
}
export interface Collection {
  id: string
  project_id: string
  name: string
  cover_asset_id: string | null
  created_at: string
}
export interface CollectionDetail extends Collection {
  assets: Asset[]
}

export const listCollections = (projectId: string) =>
  request<CollectionSummary[]>(`/projects/${projectId}/collections`)

export const createCollection = (projectId: string, name: string) =>
  request<Collection>(`/projects/${projectId}/collections`, {
    method: 'POST',
    body: JSON.stringify({ name }),
  })

export const getCollection = (id: string) => request<CollectionDetail>(`/collections/${id}`)

export const addToCollection = (id: string, assetIds: string[]) =>
  request<void>(`/collections/${id}/items`, {
    method: 'POST',
    body: JSON.stringify({ asset_ids: assetIds }),
  })

export const removeFromCollection = (id: string, assetId: string) =>
  request<void>(`/collections/${id}/items/${assetId}`, { method: 'DELETE' })

export const deleteCollection = (id: string) =>
  request<void>(`/collections/${id}`, { method: 'DELETE' })

// ── Search / RAG ─────────────────────────────────────────────────────────────
export interface ScoredAsset extends Asset {
  score: number
}

/** Smart (semantic/keyword) search over the project's assets. */
export const searchAssets = (projectId: string, q: string) =>
  request<ScoredAsset[]>(`/projects/${projectId}/assets/search?q=${encodeURIComponent(q)}`)

/** Pre-generate dedup: assets close to this prompt that already exist. */
export const similarCheck = (projectId: string, prompt: string) =>
  request<ScoredAsset[]>(`/projects/${projectId}/assets/similar-check`, {
    method: 'POST',
    body: JSON.stringify({ prompt }),
  })

/** Assets visually similar to a given one. */
export const similarAssets = (assetId: string) =>
  request<ScoredAsset[]>(`/assets/${assetId}/similar`)

/** How well an asset matches the project's approved style (0–1), or null. */
export const styleFit = (assetId: string) =>
  request<{ score: number | null; basis: number }>(`/assets/${assetId}/style-fit`)

/** Index any assets in the project lacking an embedding (covers imports). */
export const backfillEmbeddings = (projectId: string) =>
  request<{ indexed: number }>(`/projects/${projectId}/embeddings/backfill`, { method: 'POST' })

export interface ContextHit {
  source_kind: string // 'brief' | 'asset_prompt' | 'comment' | 'canon'
  source_id: string | null
  content: string
  score: number
}

export interface ContextAnswer {
  answer: string // LLM-synthesized answer (or a mock/empty note)
  sources: ContextHit[]
}

/** Ask the project — a synthesized answer over the most relevant context. */
export const askContext = (projectId: string, q: string) =>
  request<ContextAnswer>(`/projects/${projectId}/context?q=${encodeURIComponent(q)}`)

/** (Re)build the semantic-context index from briefs/prompts/comments/canon. */
export const backfillContext = (projectId: string) =>
  request<{ indexed: number }>(`/projects/${projectId}/context/backfill`, { method: 'POST' })

// ── Activity feed ────────────────────────────────────────────────────────────
export interface ActivityEvent {
  kind: 'asset' | 'comment' | 'canon'
  at: string
  summary: string
  asset_id: string | null
}

export const getActivity = (projectId: string) =>
  request<ActivityEvent[]>(`/projects/${projectId}/activity`)

// ── Generation recipes (reusable derivation templates) ──────────────────────
export interface Recipe {
  id: string
  project_id: string
  name: string
  instruction: string
  created_at: string
}

export const listRecipes = (projectId: string) =>
  request<Recipe[]>(`/projects/${projectId}/recipes`)

export const createRecipe = (projectId: string, name: string, instruction: string) =>
  request<Recipe>(`/projects/${projectId}/recipes`, {
    method: 'POST',
    body: JSON.stringify({ name, instruction }),
  })

export const deleteRecipe = (id: string) =>
  request<void>(`/recipes/${id}`, { method: 'DELETE' })

// ── Comments (collaboration) ─────────────────────────────────────────────────
export interface AssetComment {
  id: string
  asset_id: string
  author_id: string | null
  author_email: string | null
  body: string
  created_at: string
}

export const listComments = (assetId: string) =>
  request<AssetComment[]>(`/assets/${assetId}/comments`)

export const addComment = (assetId: string, body: string) =>
  request<AssetComment>(`/assets/${assetId}/comments`, {
    method: 'POST',
    body: JSON.stringify({ body }),
  })

export const deleteComment = (id: string) =>
  request<void>(`/comments/${id}`, { method: 'DELETE' })

// ── Lineage + canon propagation ──────────────────────────────────────────────
export interface AssetLink {
  from_asset: string // the derivative
  to_asset: string // the base it came from
  relation: string // 'derived_from'
}
export interface LineageGraph {
  assets: Asset[]
  links: AssetLink[]
}

export const getLineage = (projectId: string) =>
  request<LineageGraph>(`/projects/${projectId}/lineage`)

/** Rebind assets to the current canon — the "keep" side of canon propagation. */
export const reconcileAssets = (projectId: string, assetIds: string[]) =>
  request<Asset[]>(`/projects/${projectId}/reconcile`, {
    method: 'POST',
    body: JSON.stringify({ asset_ids: assetIds }),
  })

// ── Usage (shared OpenRouter key budget) ─────────────────────────────────────
export interface Usage {
  remaining: number
  usage: number
  limit: number | null
  /** "openrouter" (live), "mock" (no key), or "stale" (last good value). */
  source: string
}
/** The shared dev key's remaining OpenRouter credit. */
export const getUsage = () => request<Usage>('/usage')

// ── Export ───────────────────────────────────────────────────────────────────
export interface AssetCheck {
  id: string
  filename: string
  role: string | null
  group: string
  tags: string[]
  status: AssetStatus
  format: string | null
  width: number | null
  height: number | null
  has_alpha: boolean | null
  issues: string[]
  ok: boolean
}
export interface ExportReport {
  assets: AssetCheck[]
  ok_count: number
  issue_count: number
}

/** Pre-export deterministic check report (no pack produced). */
export const checkExport = (projectId: string, assetIds: string[]) =>
  request<ExportReport>(`/projects/${projectId}/export/check`, {
    method: 'POST',
    body: JSON.stringify({ asset_ids: assetIds }),
  })

/**
 * Build the pack and trigger a browser download of the zip. `target` selects an
 * engine pack (e.g. `'godot'`) when the project's vertical supports it; omit it
 * (or pass `'generic'`) for the vertical-neutral pack.
 */
export async function downloadExport(
  projectId: string,
  assetIds: string[],
  target?: string,
): Promise<void> {
  const res = await fetch(`${BASE}/projects/${projectId}/export`, {
    method: 'POST',
    credentials: 'include',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ asset_ids: assetIds, target }),
  })
  if (!res.ok) {
    const body = await res.json().catch(() => null)
    throw new ApiError(
      res.status,
      (body && typeof body === 'object' && 'error' in body && String(body.error)) ||
        `export failed (${res.status})`,
    )
  }
  const blob = await res.blob()
  const cd = res.headers.get('content-disposition') ?? ''
  const name = /filename="?([^"]+)"?/.exec(cd)?.[1] ?? 'pack.zip'
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = name
  document.body.appendChild(a)
  a.click()
  a.remove()
  URL.revokeObjectURL(url)
}
