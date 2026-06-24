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
