import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { PlusIcon, FolderDashedIcon, SpinnerGapIcon } from '@phosphor-icons/react'
import * as api from '../lib/api'
import { ApiError } from '../lib/api'
import { AppShell } from './AppShell'
import { ProjectCard } from './ProjectCard'

export function WorkspaceHub() {
  const [workspace, setWorkspace] = useState<api.Workspace | null>(null)
  const [projects, setProjects] = useState<api.Project[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [search, setSearch] = useState('')
  const [creating, setCreating] = useState(false)
  const [newName, setNewName] = useState('')
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    let active = true
    ;(async () => {
      try {
        const ws = await api.listWorkspaces()
        if (!active) return
        const first = ws[0] ?? null
        setWorkspace(first)
        if (first) setProjects(await api.listProjects(first.id))
      } catch (err) {
        if (active) setError(err instanceof ApiError ? err.message : 'Failed to load.')
      } finally {
        if (active) setLoading(false)
      }
    })()
    return () => {
      active = false
    }
  }, [])

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase()
    return q ? projects.filter((p) => p.name.toLowerCase().includes(q)) : projects
  }, [projects, search])

  async function onCreate(e: FormEvent) {
    e.preventDefault()
    if (!workspace || !newName.trim()) return
    setBusy(true)
    try {
      const created = await api.createProject(workspace.id, newName.trim())
      setProjects((prev) => [created, ...prev])
      setNewName('')
      setCreating(false)
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Failed to create project.')
    } finally {
      setBusy(false)
    }
  }

  return (
    <AppShell search={search} onSearch={setSearch}>
      <div className="mx-auto max-w-[1400px]">
        <div className="mb-6 flex items-end justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight text-text">
              {workspace?.name ?? 'Workspace'}
            </h1>
            <p className="mt-1 text-sm text-text-dim">
              {projects.length} {projects.length === 1 ? 'project' : 'projects'}
            </p>
          </div>
          <button
            onClick={() => setCreating((v) => !v)}
            className="inline-flex items-center gap-2 rounded-[10px] bg-teal px-4 py-2.5 text-sm font-semibold text-bg transition active:translate-y-px"
          >
            <PlusIcon size={16} weight="bold" /> New project
          </button>
        </div>

        {creating && (
          <form onSubmit={onCreate} className="glass mb-6 flex gap-2 rounded-[12px] p-2">
            <input
              autoFocus
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="Project name"
              className="flex-1 rounded-[8px] bg-surface-2/60 px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-2 focus:ring-teal/30"
            />
            <button
              type="submit"
              disabled={busy || !newName.trim()}
              className="inline-flex items-center gap-2 rounded-[8px] bg-indigo px-4 py-2 text-sm font-semibold text-white transition active:translate-y-px disabled:opacity-60"
            >
              {busy && <SpinnerGapIcon size={15} className="animate-spin" />}
              Create
            </button>
          </form>
        )}

        {error && (
          <p className="mb-4 rounded-[10px] border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-sm text-rose-300">
            {error}
          </p>
        )}

        {loading ? (
          <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {Array.from({ length: 8 }).map((_, i) => (
              <div key={i} className="glass h-56 animate-pulse rounded-[16px]" />
            ))}
          </div>
        ) : filtered.length === 0 ? (
          <div className="glass mt-4 grid place-items-center rounded-[16px] px-6 py-20 text-center">
            <span className="mb-4 grid size-14 place-items-center rounded-full bg-white/5 text-text-dim">
              <FolderDashedIcon size={28} />
            </span>
            <p className="text-text">
              {search ? 'No projects match your search.' : 'No projects yet.'}
            </p>
            {!search && (
              <p className="mt-1 text-sm text-text-dim">
                Create your first project to start designing.
              </p>
            )}
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {filtered.map((p) => (
              <ProjectCard key={p.id} project={p} />
            ))}
          </div>
        )}
      </div>
    </AppShell>
  )
}
