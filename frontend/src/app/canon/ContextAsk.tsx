import { useState, type FormEvent } from 'react'
import { MagnifyingGlassIcon, SpinnerGapIcon, BrainIcon, ArrowsClockwiseIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'

const KIND_LABEL: Record<string, string> = {
  brief: 'brief',
  asset_prompt: 'asset',
  comment: 'comment',
  canon: 'canon',
}

/**
 * "Ask this project" — semantic retrieval over the project's text (brief, asset
 * prompts, comments, canon) to answer "why was this made / what's it for".
 * Retrieval-only for now; the snippets are the answer. Reindex covers imports
 * and anything added before indexing.
 */
export function ContextAsk({ projectId }: { projectId: string }) {
  const [q, setQ] = useState('')
  const [result, setResult] = useState<api.ContextAnswer | null>(null)
  const [busy, setBusy] = useState(false)
  const [reindexing, setReindexing] = useState(false)
  const [note, setNote] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function ask(e: FormEvent) {
    e.preventDefault()
    const query = q.trim()
    if (!query || busy) return
    setBusy(true)
    setError(null)
    setNote(null)
    try {
      setResult(await api.askContext(projectId, query))
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Ask failed.')
    } finally {
      setBusy(false)
    }
  }

  async function reindex() {
    if (reindexing) return
    setReindexing(true)
    setError(null)
    try {
      const r = await api.backfillContext(projectId)
      setNote(`Indexed ${r.indexed} snippets.`)
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Reindex failed.')
    } finally {
      setReindexing(false)
    }
  }

  return (
    <div className="glass flex max-h-[42%] shrink-0 flex-col rounded-[16px]">
      <div className="flex items-center gap-2 border-b border-white/8 px-5 py-3">
        <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
          <BrainIcon size={15} weight="fill" />
        </span>
        <p className="text-sm font-medium text-text">Ask this project</p>
        <button
          onClick={reindex}
          disabled={reindexing}
          title="Rebuild the context index (covers imports + recent changes)"
          className="ml-auto inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-2.5 py-1 text-xs text-text-dim transition hover:text-text disabled:opacity-50"
        >
          {reindexing ? <SpinnerGapIcon size={12} className="animate-spin" /> : <ArrowsClockwiseIcon size={12} />}
          Reindex
        </button>
      </div>

      <form onSubmit={ask} className="border-b border-white/8 p-3">
        <div className="mx-auto flex max-w-2xl items-center gap-2 rounded-[12px] bg-surface-2/60 p-2 transition focus-within:ring-1 focus-within:ring-teal/40">
          <MagnifyingGlassIcon size={15} className="ml-1 text-text-dim" />
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Why was this made? What are the torch assets for?…"
            aria-label="Ask about this project's canon"
            className="flex-1 bg-transparent px-1 text-sm text-text outline-none placeholder:text-text-dim"
          />
          <button
            type="submit"
            disabled={busy || !q.trim()}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3 py-1.5 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
          >
            {busy ? <SpinnerGapIcon size={13} className="animate-spin" /> : 'Ask'}
          </button>
        </div>
        {(note || error) && (
          <p className={`mx-auto mt-2 max-w-2xl text-xs ${error ? 'text-rose-300' : 'text-text-dim'}`}>
            {error ?? note}
          </p>
        )}
      </form>

      <div className="min-h-0 flex-1 overflow-y-auto px-3 py-2">
        {result == null ? (
          <p className="px-2 py-6 text-center text-xs text-text-dim">
            Ask a question to get an answer synthesized from the project's context.
          </p>
        ) : result.sources.length === 0 ? (
          <p className="px-2 py-6 text-center text-xs text-text-dim">
            No relevant context found. Try Reindex if you just added assets.
          </p>
        ) : (
          <div className="mx-auto max-w-2xl space-y-3">
            <div className="rounded-[12px] border border-teal/20 bg-teal/8 px-3 py-2.5">
              <p className="whitespace-pre-wrap text-sm text-text">{result.answer}</p>
            </div>
            <div>
              <p className="mb-1.5 px-1 text-[10px] font-semibold uppercase tracking-wider text-text-dim">
                Sources
              </p>
              <ul className="space-y-1.5">
                {result.sources.map((h, i) => (
                  <li key={i} className="flex items-start gap-2 rounded-[10px] bg-white/[0.03] px-2.5 py-2">
                    <span className="mt-0.5 shrink-0 rounded-[6px] bg-white/8 px-1.5 py-0.5 text-[10px] font-medium text-text-dim">
                      {KIND_LABEL[h.source_kind] ?? h.source_kind}
                    </span>
                    <p className="min-w-0 flex-1 text-xs text-text-muted">{h.content}</p>
                    <span className="mt-0.5 shrink-0 text-[10px] tabular-nums text-text-dim">
                      {Math.round(h.score * 100)}%
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
