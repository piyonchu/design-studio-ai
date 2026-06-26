import { useEffect, useState, type FormEvent } from 'react'
import { PaletteIcon, SpinnerGapIcon, CheckIcon, ClockCounterClockwiseIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'
import { verticalConfig } from '../verticals'

type CanonData = { style?: Record<string, string>; negative?: string[] }

/**
 * Define a project's canon — the style rules every derivation is bound to.
 * Each save appends a new version (lineage handled by the backend).
 */
export function CanonView({ projectId, vertical }: { projectId: string; vertical?: string }) {
  const STYLE_FIELDS = verticalConfig(vertical).canonFields
  const [style, setStyle] = useState<Record<string, string>>({})
  const [negative, setNegative] = useState('')
  const [version, setVersion] = useState<number | null>(null)
  const [history, setHistory] = useState<api.Canon[]>([])
  const [busy, setBusy] = useState(false)
  const [saved, setSaved] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function loadHistory() {
    api.getCanonHistory(projectId).then(setHistory).catch(() => {})
  }

  useEffect(() => {
    api
      .getCanon(projectId)
      .then((c) => {
        if (!c) return
        const d = (c.data ?? {}) as CanonData
        setStyle(d.style ?? {})
        setNegative((d.negative ?? []).join('\n'))
        setVersion(c.version)
      })
      .catch(() => {})
    loadHistory()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId])

  async function save(e: FormEvent) {
    e.preventDefault()
    if (busy) return
    setBusy(true)
    setError(null)
    setSaved(false)
    try {
      const data: CanonData = {
        style,
        negative: negative
          .split('\n')
          .map((s) => s.trim())
          .filter(Boolean),
      }
      const c = await api.saveCanon(projectId, data)
      setVersion(c.version)
      setSaved(true)
      loadHistory()
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Save failed.')
    } finally {
      setBusy(false)
    }
  }

  return (
    <form onSubmit={save} className="glass flex min-h-0 flex-1 flex-col rounded-[16px]">
      <div className="flex items-center gap-2 border-b border-white/8 px-5 py-4">
        <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
          <PaletteIcon size={15} weight="fill" />
        </span>
        <p className="text-sm font-medium text-text">Canon</p>
        {version != null && <span className="text-sm text-text-dim">· v{version}</span>}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-5">
        <div className="mx-auto grid max-w-2xl gap-4">
          {STYLE_FIELDS.map(([key, label, placeholder]) => (
            <label key={key} className="grid gap-1.5">
              <span className="text-xs font-medium text-text-dim">{label}</span>
              <input
                value={style[key] ?? ''}
                onChange={(e) => setStyle((s) => ({ ...s, [key]: e.target.value }))}
                placeholder={placeholder}
                className="rounded-[10px] bg-surface-2/60 px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
              />
            </label>
          ))}

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-text-dim">Negative (one per line)</span>
            <textarea
              value={negative}
              onChange={(e) => setNegative(e.target.value)}
              placeholder={'no photorealism\nno text or watermark'}
              rows={4}
              className="resize-y rounded-[10px] bg-surface-2/60 px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
            />
          </label>

          {history.length > 0 && (
            <div className="mt-2 border-t border-white/8 pt-4">
              <p className="mb-2 inline-flex items-center gap-1.5 text-xs font-medium text-text-dim">
                <ClockCounterClockwiseIcon size={14} />
                Version history
              </p>
              <ol className="space-y-1.5">
                {history.map((c) => (
                  <li key={c.id} className="flex items-start gap-2.5 rounded-[10px] bg-surface-2/40 px-3 py-2">
                    <span
                      className={`mt-px shrink-0 rounded-[6px] px-1.5 py-0.5 text-[10px] font-semibold ${
                        c.version === version ? 'bg-teal/20 text-teal-bright' : 'bg-white/8 text-text-dim'
                      }`}
                    >
                      v{c.version}
                    </span>
                    <p className="min-w-0 flex-1 text-xs text-text-muted">
                      {c.change_note ?? <span className="text-text-dim">no change note</span>}
                    </p>
                    <span className="mt-px shrink-0 text-[10px] text-text-dim">
                      {new Date(c.created_at).toLocaleDateString()}
                    </span>
                  </li>
                ))}
              </ol>
            </div>
          )}
        </div>
      </div>

      <div className="flex items-center gap-3 border-t border-white/8 px-5 py-3">
        <button
          type="submit"
          disabled={busy}
          className="inline-flex items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
        >
          {busy ? (
            <SpinnerGapIcon size={14} className="animate-spin" />
          ) : (
            <CheckIcon size={14} weight="bold" />
          )}
          {version == null ? 'Save canon' : 'Save new version'}
        </button>
        {saved && <span className="text-xs text-teal-bright">Saved · v{version}</span>}
        {error && <span className="text-xs text-rose-300">{error}</span>}
      </div>
    </form>
  )
}
