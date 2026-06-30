import { useEffect, useRef, useState, type FormEvent } from 'react'
import { PaperPlaneRightIcon, SpinnerGapIcon, TrashIcon, ChatCircleIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { formatApiError } from '../../lib/api'
import { useAuth } from '../../auth/AuthContext'

/** Relative time, e.g. "just now", "4m", "2h", "3d", else a short date. */
function ago(iso: string): string {
  const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000)
  if (s < 45) return 'just now'
  if (s < 3600) return `${Math.round(s / 60)}m`
  if (s < 86400) return `${Math.round(s / 3600)}h`
  if (s < 604800) return `${Math.round(s / 86400)}d`
  return new Date(iso).toLocaleDateString()
}

/**
 * The comment thread for one asset — shared by the inspector and the review
 * queue. Loads on `assetId` change, posts new comments, and lets an author
 * delete their own. `onCountChange` lets a parent show a live badge.
 */
export function CommentThread({
  assetId,
  onCountChange,
}: {
  assetId: string
  onCountChange?: (n: number) => void
}) {
  const { user } = useAuth()
  const [comments, setComments] = useState<api.AssetComment[]>([])
  const [draft, setDraft] = useState('')
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const endRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    let alive = true
    setLoading(true)
    setError(null)
    api
      .listComments(assetId)
      .then((cs) => {
        if (!alive) return
        setComments(cs)
        onCountChange?.(cs.length)
      })
      .catch((err) => alive && setError(formatApiError(err, "Couldn't load comments. Try again.")))
      .finally(() => alive && setLoading(false))
    return () => {
      alive = false
    }
    // onCountChange intentionally omitted — parents pass an inline callback.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [assetId])

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: 'nearest' })
  }, [comments.length])

  async function submit(e: FormEvent) {
    e.preventDefault()
    const body = draft.trim()
    if (!body || busy) return
    setBusy(true)
    setError(null)
    try {
      const created = await api.addComment(assetId, body)
      setComments((c) => {
        const next = [...c, created]
        onCountChange?.(next.length)
        return next
      })
      setDraft('')
    } catch (err) {
      setError(formatApiError(err, "Couldn't post your comment. Try again."))
    } finally {
      setBusy(false)
    }
  }

  async function remove(id: string) {
    try {
      await api.deleteComment(id)
      setComments((c) => {
        const next = c.filter((x) => x.id !== id)
        onCountChange?.(next.length)
        return next
      })
    } catch (err) {
      setError(formatApiError(err, "Couldn't delete that comment. Try again."))
    }
  }

  return (
    <div className="flex min-h-0 flex-col">
      <div className="flex items-center gap-1.5 text-xs text-text-dim">
        <ChatCircleIcon size={14} />
        Discussion
        {comments.length > 0 && <span className="text-text-dim">· {comments.length}</span>}
      </div>

      <div className="mt-3 min-h-0 flex-1 space-y-3 overflow-y-auto">
        {loading ? (
          <p className="text-xs text-text-dim">Loading comments…</p>
        ) : comments.length === 0 ? (
          <p className="text-xs text-text-dim">No comments yet. Add context for reviewers below.</p>
        ) : (
          comments.map((c) => (
            <div key={c.id} className="group">
              <div className="flex items-baseline gap-2">
                <span className="truncate text-xs font-medium text-text">{c.author_email ?? 'unknown'}</span>
                <span className="text-[10px] text-text-dim">{ago(c.created_at)}</span>
                {c.author_id && user?.id === c.author_id && (
                  <button
                    onClick={() => remove(c.id)}
                    aria-label="Delete comment"
                    className="ml-auto text-text-dim opacity-0 transition hover:text-rose-300 group-hover:opacity-100"
                  >
                    <TrashIcon size={12} />
                  </button>
                )}
              </div>
              <p className="mt-0.5 whitespace-pre-wrap break-words text-xs text-text-muted">{c.body}</p>
            </div>
          ))
        )}
        <div ref={endRef} />
      </div>

      {error && <p className="mt-2 text-xs text-rose-300">{error}</p>}

      <form onSubmit={submit} className="mt-3 flex items-center gap-2 rounded-[10px] bg-surface/60 p-1.5">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="Add a comment…"
          className="flex-1 bg-transparent px-2 text-xs text-text outline-none placeholder:text-text-dim"
        />
        <button
          type="submit"
          disabled={busy || !draft.trim()}
          aria-label="Post comment"
          className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-teal text-bg transition active:translate-y-px disabled:opacity-40"
        >
          {busy ? <SpinnerGapIcon size={13} className="animate-spin" /> : <PaperPlaneRightIcon size={13} weight="fill" />}
        </button>
      </form>
    </div>
  )
}
