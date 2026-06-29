import { useEffect, useState } from 'react'
import {
  ClockCounterClockwiseIcon,
  ArrowsClockwiseIcon,
  SpinnerGapIcon,
  ArrowUUpLeftIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'

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
 * Per-asset version history (A2) — the pro pipeline's headline. Lists every
 * version (note · author · time), regenerates into a new version, rolls back
 * (non-destructively, as a new head), and shows a before/after slider diff
 * between the current head and any prior version.
 */
export function VersionHistory({
  asset,
  onChanged,
}: {
  asset: api.Asset
  onChanged: (a: api.Asset) => void
}) {
  const [versions, setVersions] = useState<api.AssetVersion[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [showRegen, setShowRegen] = useState(false)
  const [regenPrompt, setRegenPrompt] = useState(asset.prompt ?? '')
  const [compareId, setCompareId] = useState<string | null>(null)
  const [slider, setSlider] = useState(50)

  const isImage = asset.kind !== 'audio'

  function reload() {
    api.listVersions(asset.id).then(setVersions).catch(() => {})
  }
  // Reload whenever the head moves (after regenerate / restore).
  useEffect(reload, [asset.id, asset.current_version_id])

  const head = versions.find((v) => v.id === asset.current_version_id) ?? versions[0]
  const compare = versions.find((v) => v.id === compareId)
  const showDiff = isImage && compare && head && compare.id !== head.id

  async function regenerate() {
    if (busy) return
    setBusy(true)
    setError(null)
    try {
      const updated = await api.regenerateAsset(asset.id, regenPrompt.trim() || undefined)
      onChanged(updated)
      setShowRegen(false)
      // head change triggers reload via effect; nudge in case id is reused
      reload()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Regenerate failed.')
    } finally {
      setBusy(false)
    }
  }

  async function restore(versionId: string) {
    if (busy) return
    setBusy(true)
    setError(null)
    try {
      const updated = await api.restoreVersion(asset.id, versionId)
      onChanged(updated)
      setCompareId(null)
      reload()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Restore failed.')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div>
      <div className="flex items-center gap-1.5 text-xs text-text-dim">
        <ClockCounterClockwiseIcon size={14} />
        History
        {versions.length > 0 && <span className="tabular-nums">· {versions.length}</span>}
        {isImage && (
          <button
            onClick={() => {
              setShowRegen((s) => !s)
              setRegenPrompt(asset.prompt ?? '')
            }}
            className="ml-auto inline-flex items-center gap-1 rounded-[7px] border border-white/10 px-2 py-1 text-[11px] text-text-dim transition hover:text-text"
          >
            <ArrowsClockwiseIcon size={12} />
            Regenerate
          </button>
        )}
      </div>

      {showRegen && (
        <div className="mt-2 rounded-[10px] bg-surface/60 p-2">
          <textarea
            value={regenPrompt}
            onChange={(e) => setRegenPrompt(e.target.value)}
            rows={2}
            placeholder="Prompt for the new version…"
            className="w-full resize-none rounded-[8px] bg-surface-2/60 px-2.5 py-2 text-xs text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
          />
          <div className="mt-1.5 flex items-center gap-2">
            <button
              onClick={regenerate}
              disabled={busy}
              className="inline-flex items-center gap-1.5 rounded-[8px] bg-teal px-3 py-1.5 text-xs font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
            >
              {busy ? <SpinnerGapIcon size={12} className="animate-spin" /> : <ArrowsClockwiseIcon size={12} weight="fill" />}
              Create new version
            </button>
            <span className="text-[10px] text-text-dim">conditions on canon + exemplar</span>
          </div>
        </div>
      )}

      {error && <p className="mt-2 text-xs text-rose-300">{error}</p>}

      {/* Before/after diff slider (head vs the selected prior version) */}
      {showDiff && compare && head && (
        <div className="mt-3">
          <p className="mb-1.5 flex items-center justify-between text-[11px] text-text-dim">
            <span>
              Diff: <span className="text-text">v{compare.version}</span> → <span className="text-teal-bright">v{head.version}</span>
            </span>
            <button onClick={() => setCompareId(null)} className="hover:text-text">
              Close
            </button>
          </p>
          <div className="relative aspect-square w-full select-none overflow-hidden rounded-[10px] ring-1 ring-white/10">
            {/* head (after) underneath */}
            <img src={head.url} alt={`v${head.version}`} className="absolute inset-0 size-full object-contain" />
            {/* prior (before) clipped to the slider position on top */}
            <img
              src={compare.url}
              alt={`v${compare.version}`}
              className="absolute inset-0 size-full object-contain"
              style={{ clipPath: `inset(0 ${100 - slider}% 0 0)` }}
            />
            <div className="pointer-events-none absolute inset-y-0 w-0.5 bg-teal/80" style={{ left: `${slider}%` }} />
            <span className="absolute left-1.5 top-1.5 rounded-[5px] bg-black/55 px-1.5 py-0.5 text-[10px] text-white/90 backdrop-blur">
              v{compare.version}
            </span>
            <span className="absolute right-1.5 top-1.5 rounded-[5px] bg-teal/80 px-1.5 py-0.5 text-[10px] text-bg">
              v{head.version}
            </span>
          </div>
          <input
            type="range"
            min={0}
            max={100}
            value={slider}
            onChange={(e) => setSlider(Number(e.target.value))}
            aria-label="Compare slider"
            className="mt-2 w-full accent-teal"
          />
        </div>
      )}

      <ul className="mt-3 space-y-1.5">
        {versions.map((v) => {
          const isHead = head?.id === v.id
          const isCompare = compareId === v.id
          return (
            <li
              key={v.id}
              className={`flex items-center gap-2.5 rounded-[10px] p-1.5 transition ${
                isCompare ? 'bg-teal/10 ring-1 ring-teal/30' : 'hover:bg-white/5'
              }`}
            >
              {isImage ? (
                <button
                  onClick={() => setCompareId(isHead ? null : isCompare ? null : v.id)}
                  title={isHead ? 'Current version' : 'Compare with current'}
                  className="size-11 shrink-0 overflow-hidden rounded-[8px] ring-1 ring-white/10 transition hover:ring-teal"
                >
                  <img src={v.url} alt={`v${v.version}`} className="size-full object-cover" />
                </button>
              ) : (
                <span className="grid size-11 shrink-0 place-items-center rounded-[8px] bg-surface/60 text-[11px] font-semibold text-text-dim ring-1 ring-white/10">
                  v{v.version}
                </span>
              )}
              <div className="min-w-0 flex-1">
                <p className="flex items-center gap-1.5 text-xs text-text">
                  <span className="font-semibold">v{v.version}</span>
                  {isHead && (
                    <span className="rounded-[5px] bg-teal/20 px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wide text-teal-bright">
                      current
                    </span>
                  )}
                  <span className="truncate text-text-dim">{v.change_note ?? 'Initial'}</span>
                </p>
                <p className="truncate text-[10px] text-text-dim">
                  {v.author_email ?? 'system'} · {ago(v.created_at)}
                </p>
              </div>
              {!isHead && (
                <button
                  onClick={() => restore(v.id)}
                  disabled={busy}
                  title={`Roll back to v${v.version}`}
                  className="inline-flex shrink-0 items-center gap-1 rounded-[7px] border border-white/10 px-2 py-1 text-[11px] text-text-dim transition hover:text-text disabled:opacity-40"
                >
                  <ArrowUUpLeftIcon size={12} />
                  Restore
                </button>
              )}
            </li>
          )
        })}
      </ul>
    </div>
  )
}
