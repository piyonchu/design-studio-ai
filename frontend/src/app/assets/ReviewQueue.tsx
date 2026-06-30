import { useEffect, useMemo, useState } from 'react'
import { CheckIcon, XIcon, FlagIcon, SpinnerGapIcon, TrayIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'
import { CommentThread } from './CommentThread'

/**
 * Review queue — the candidates awaiting a decision as a focused worklist.
 * Left: the pending stack. Right: the focused candidate with a large preview,
 * approve / needs-review / reject, and its discussion thread. A decision drops
 * the asset from the queue and advances to the next, so a reviewer can clear
 * the backlog without leaving the panel.
 */
export function ReviewQueue({ projectId, canApprove = true }: { projectId: string; canApprove?: boolean }) {
  const [queue, setQueue] = useState<api.Asset[]>([])
  const [focusId, setFocusId] = useState<string | null>(null)
  const [fit, setFit] = useState<{ score: number | null; basis: number } | null>(null)
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  // Style-fit of the focused candidate vs the project's approved assets.
  useEffect(() => {
    if (!focusId) {
      setFit(null)
      return
    }
    let alive = true
    setFit(null)
    api.styleFit(focusId).then((f) => alive && setFit(f)).catch(() => {})
    return () => {
      alive = false
    }
  }, [focusId])

  useEffect(() => {
    let alive = true
    api
      // Server-filtered to the pending statuses; a worklist rarely exceeds 100.
      .listAssets(projectId, { status: ['candidate', 'needs_review'], limit: 100 })
      .then((page) => {
        if (!alive) return
        const pending = page.items
        setQueue(pending)
        setFocusId((id) => id ?? pending[0]?.id ?? null)
      })
      .catch(() => alive && setError('Failed to load the queue.'))
      .finally(() => alive && setLoading(false))
    return () => {
      alive = false
    }
  }, [projectId])

  const focused = useMemo(() => queue.find((a) => a.id === focusId) ?? null, [queue, focusId])

  async function decide(id: string, status: api.AssetStatus) {
    if (busy) return
    setBusy(true)
    setError(null)
    try {
      await api.setAssetStatus(id, status)
      setQueue((q) => {
        const next = q.filter((a) => a.id !== id)
        // Advance focus to the neighbour that takes this slot, else the last.
        if (id === focusId) {
          const idx = q.findIndex((a) => a.id === id)
          setFocusId(next[idx]?.id ?? next[idx - 1]?.id ?? null)
        }
        return next
      })
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Update failed.')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="glass flex min-h-0 flex-1 overflow-hidden rounded-[16px]">
      {/* Pending stack */}
      <aside className="flex w-64 shrink-0 flex-col border-r border-white/8">
        <div className="flex items-center gap-2 border-b border-white/8 px-4 py-4">
          <span className="grid size-7 place-items-center rounded-[8px] bg-amber-400/15 text-amber-300">
            <TrayIcon size={15} weight="fill" />
          </span>
          <p className="text-sm font-medium text-text">Review Queue</p>
          <span className="text-sm text-text-dim">· {queue.length}</span>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          {loading ? (
            <p className="px-2 py-8 text-center text-xs text-text-dim">Loading…</p>
          ) : queue.length === 0 ? (
            <p className="px-2 py-12 text-center text-sm text-text-dim">All caught up — nothing to review.</p>
          ) : (
            queue.map((a) => (
              <button
                key={a.id}
                onClick={() => setFocusId(a.id)}
                className={`mb-1 flex w-full items-center gap-2.5 rounded-[10px] p-1.5 text-left transition ${
                  a.id === focusId ? 'bg-teal/12 ring-1 ring-teal/40' : 'hover:bg-white/5'
                }`}
              >
                <img src={a.url} alt="" loading="lazy" decoding="async" className="size-11 shrink-0 rounded-[8px] object-cover ring-1 ring-white/10" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-xs text-text">{api.displayName(a)}</span>
                  <span className="mt-0.5 inline-flex items-center gap-1 text-[10px] text-text-dim">
                    <span className={`size-1.5 rounded-full ${a.status === 'needs_review' ? 'bg-rose-400' : 'bg-amber-400'}`} />
                    {a.status.replace('_', ' ')} · {a.source_kind}
                  </span>
                </span>
              </button>
            ))
          )}
        </div>
      </aside>

      {/* Focused candidate */}
      <div className="flex min-h-0 flex-1 flex-col">
        {!focused ? (
          <div className="grid flex-1 place-items-center px-6 text-center text-sm text-text-dim">
            {queue.length === 0 ? 'No candidates pending. Generate or derive assets to review them here.' : 'Select a candidate.'}
          </div>
        ) : (
          <div className="flex min-h-0 flex-1">
            <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-4 p-6">
              <img
                src={focused.url}
                alt={focused.role ?? ''}
                className="max-h-[52vh] max-w-full rounded-[14px] object-contain ring-1 ring-white/10"
              />
              {fit?.score != null && (
                <span
                  title={`Embedding similarity to the nearest of ${fit.basis} approved asset(s)`}
                  className={`rounded-[8px] px-2.5 py-1 text-xs font-medium ${
                    fit.score >= 0.75
                      ? 'bg-teal/15 text-teal-bright'
                      : fit.score >= 0.5
                        ? 'bg-amber-400/15 text-amber-200'
                        : 'bg-rose-500/15 text-rose-200'
                  }`}
                >
                  Style fit {Math.round(fit.score * 100)}%
                </span>
              )}
              <div className="flex items-center gap-2">
                <button
                  onClick={() => decide(focused.id, 'approved')}
                  disabled={busy || !canApprove}
                  title={canApprove ? undefined : 'Only a reviewer or owner can approve'}
                  className="inline-flex items-center gap-1.5 rounded-[10px] bg-teal px-4 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
                >
                  {busy ? <SpinnerGapIcon size={15} className="animate-spin" /> : <CheckIcon size={15} weight="bold" />}
                  Approve
                </button>
                <button
                  onClick={() => decide(focused.id, 'needs_review')}
                  disabled={busy}
                  className="inline-flex items-center gap-1.5 rounded-[10px] border border-white/10 px-4 py-2 text-sm text-amber-200 transition hover:bg-white/5 disabled:opacity-50"
                >
                  <FlagIcon size={15} weight="fill" />
                  Needs review
                </button>
                <button
                  onClick={() => decide(focused.id, 'rejected')}
                  disabled={busy}
                  className="inline-flex items-center gap-1.5 rounded-[10px] border border-white/10 px-4 py-2 text-sm text-rose-200 transition hover:bg-white/5 disabled:opacity-50"
                >
                  <XIcon size={15} weight="bold" />
                  Reject
                </button>
              </div>
              {error && <p className="text-xs text-rose-300">{error}</p>}
              <p className="max-w-md text-center text-xs text-text-dim">
                {focused.derivation ?? focused.prompt ?? focused.role ?? ''}
              </p>
            </div>

            {/* Discussion alongside the decision */}
            <div className="flex w-80 shrink-0 flex-col border-l border-white/8 p-4">
              <CommentThread assetId={focused.id} />
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
