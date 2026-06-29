import { useEffect, useState } from 'react'
import { ArrowCounterClockwiseIcon, SpinnerGapIcon, TrashIcon } from '@phosphor-icons/react'
import * as api from '../lib/api'
import { ApiError } from '../lib/api'
import { AppShell } from './AppShell'
import { verticalConfig } from './verticals'

/**
 * Trash — soft-deleted projects for the workspace, with one-click restore.
 * Projects are moved here from the hub (delete) and stay recoverable.
 */
export function TrashPage() {
  const [projects, setProjects] = useState<api.Project[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)

  useEffect(() => {
    let active = true
    ;(async () => {
      try {
        const list = await api.listWorkspaces()
        const first = list[0] ?? null
        if (!active) return
        if (first) setProjects(await api.listTrash(first.id))
      } catch (e) {
        if (active) setError(e instanceof ApiError ? e.message : 'Failed to load trash.')
      } finally {
        if (active) setLoading(false)
      }
    })()
    return () => {
      active = false
    }
  }, [])

  async function restore(id: string) {
    setBusy(id)
    setError(null)
    try {
      await api.restoreProject(id)
      setProjects((p) => p.filter((x) => x.id !== id))
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Restore failed.')
    } finally {
      setBusy(null)
    }
  }

  return (
    <AppShell>
      <div className="mx-auto max-w-[900px]">
        <div className="mb-6">
          <h1 className="text-2xl font-semibold tracking-tight text-text">Trash</h1>
          <p className="mt-1 text-sm text-text-dim">
            Deleted projects stay here and can be restored.
          </p>
        </div>

        {error && <p className="mb-3 text-sm text-rose-300">{error}</p>}

        {loading ? (
          <div className="grid place-items-center py-16 text-text-dim">
            <SpinnerGapIcon size={20} className="animate-spin" />
          </div>
        ) : projects.length === 0 ? (
          <div className="glass grid place-items-center gap-2 rounded-[16px] py-16 text-text-dim">
            <TrashIcon size={28} />
            <p className="text-sm">Trash is empty.</p>
          </div>
        ) : (
          <ul className="space-y-2">
            {projects.map((p) => (
              <li key={p.id} className="glass flex items-center gap-3 rounded-[12px] px-4 py-3">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium text-text">{p.name}</p>
                  <p className="truncate text-xs text-text-dim">
                    {verticalConfig(p.vertical).label}
                    {p.deleted_at ? ` · deleted ${new Date(p.deleted_at).toLocaleDateString()}` : ''}
                  </p>
                </div>
                <button
                  onClick={() => restore(p.id)}
                  disabled={busy === p.id}
                  className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-text transition hover:bg-white/5 disabled:opacity-50"
                >
                  {busy === p.id ? (
                    <SpinnerGapIcon size={14} className="animate-spin" />
                  ) : (
                    <ArrowCounterClockwiseIcon size={14} />
                  )}
                  Restore
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </AppShell>
  )
}
