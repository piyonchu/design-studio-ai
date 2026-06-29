import { useEffect, useState } from 'react'
import { XIcon, SpinnerGapIcon, TreeStructureIcon, CheckIcon, TrashIcon, MusicNotesIcon, StarIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'
import { CommentThread } from './CommentThread'

/**
 * Asset inspector — a slide-over for one asset: preview, editable role/tags,
 * provenance, and the lineage strip (base it came from + its derivatives).
 * `onNavigate` lets you hop to a related asset; `onChanged` syncs the board.
 */
export function AssetInspector({
  assetId,
  onClose,
  onNavigate,
  onChanged,
  onDeleted,
}: {
  assetId: string | null
  onClose: () => void
  onNavigate: (id: string) => void
  onChanged: (asset: api.Asset) => void
  onDeleted: (id: string) => void
}) {
  const [detail, setDetail] = useState<api.AssetDetail | null>(null)
  const [name, setName] = useState('')
  const [role, setRole] = useState('')
  const [tags, setTags] = useState('')
  const [busy, setBusy] = useState(false)
  const [saved, setSaved] = useState(false)
  const [confirming, setConfirming] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [collections, setCollections] = useState<api.CollectionSummary[]>([])
  const [selectedCol, setSelectedCol] = useState('')
  const [added, setAdded] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!assetId) return
    let alive = true
    setDetail(null)
    setError(null)
    setSaved(false)
    setConfirming(false)
    setDeleting(false)
    setSelectedCol('')
    setAdded(null)
    api
      .getAsset(assetId)
      .then((d) => {
        if (!alive) return
        setDetail(d)
        setName(d.name ?? '')
        setRole(d.role ?? '')
        setTags(d.tags.join(', '))
        api.listCollections(d.project_id).then((cs) => alive && setCollections(cs)).catch(() => {})
      })
      .catch((e) => alive && setError(e instanceof ApiError ? e.message : 'Failed to load.'))
    return () => {
      alive = false
    }
  }, [assetId])

  if (!assetId) return null

  async function save() {
    if (!detail || busy) return
    setBusy(true)
    setError(null)
    setSaved(false)
    try {
      const updated = await api.updateAsset(detail.id, {
        name: name.trim(),
        role: role.trim(),
        tags: tags.split(',').map((t) => t.trim()).filter(Boolean),
      })
      setDetail((d) => (d ? { ...d, ...updated } : d))
      onChanged(updated)
      setSaved(true)
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Save failed.')
    } finally {
      setBusy(false)
    }
  }

  async function toggleExemplar() {
    if (!detail || busy) return
    setBusy(true)
    setError(null)
    try {
      const updated = await api.updateAsset(detail.id, { exemplar: !detail.exemplar })
      setDetail((d) => (d ? { ...d, ...updated } : d))
      onChanged(updated)
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Update failed.')
    } finally {
      setBusy(false)
    }
  }

  async function addTo() {
    if (!detail || !selectedCol) return
    try {
      await api.addToCollection(selectedCol, [detail.id])
      setAdded(collections.find((c) => c.id === selectedCol)?.name ?? 'collection')
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Add failed.')
    }
  }

  async function remove() {
    if (!detail || deleting) return
    setDeleting(true)
    setError(null)
    try {
      await api.deleteAsset(detail.id)
      onDeleted(detail.id)
      onClose()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Delete failed.')
      setDeleting(false)
    }
  }

  return (
    <>
      <div className="fixed inset-0 z-40 bg-black/40" onClick={onClose} aria-hidden />
      <aside className="fixed inset-y-0 right-0 z-50 flex w-[380px] max-w-[92vw] flex-col border-l border-white/10 bg-surface-2 shadow-2xl">
        <header className="flex items-center gap-2 border-b border-white/8 px-4 py-3">
          <p className="text-sm font-medium text-text">Inspector</p>
          <button
            onClick={onClose}
            aria-label="Close"
            className="ml-auto grid size-7 place-items-center rounded-[8px] text-text-dim transition hover:bg-white/5 hover:text-text"
          >
            <XIcon size={16} />
          </button>
        </header>

        {!detail ? (
          <div className="grid flex-1 place-items-center text-text-dim">
            {error ? <p className="text-sm text-rose-300">{error}</p> : <SpinnerGapIcon size={20} className="animate-spin" />}
          </div>
        ) : (
          <div className="min-h-0 flex-1 overflow-y-auto p-4">
            {detail.kind === 'audio' ? (
              <div className="mb-3 flex aspect-square w-full flex-col items-center justify-center gap-3 rounded-[12px] bg-surface/60 p-4 ring-1 ring-white/10">
                <MusicNotesIcon size={40} weight="fill" className="text-teal-bright" />
                <audio controls src={detail.url} className="w-full" />
              </div>
            ) : (
              <img
                src={detail.url}
                alt={detail.role ?? ''}
                className="mb-3 aspect-square w-full rounded-[12px] object-contain ring-1 ring-white/10"
              />
            )}

            <div className="mb-4 flex flex-wrap items-center gap-1.5 text-[11px]">
              <span className="rounded-[6px] bg-white/8 px-1.5 py-0.5 text-text-dim">{detail.source_kind}</span>
              <span className="rounded-[6px] bg-white/8 px-1.5 py-0.5 text-text-dim">{detail.status}</span>
              {detail.canon_version_id && (
                <span className="rounded-[6px] bg-white/8 px-1.5 py-0.5 text-text-dim">canon-bound</span>
              )}
              {detail.exemplar && (
                <span className="inline-flex items-center gap-1 rounded-[6px] bg-amber-400/15 px-1.5 py-0.5 text-amber-200">
                  <StarIcon size={11} weight="fill" /> exemplar
                </span>
              )}
            </div>

            <button
              onClick={toggleExemplar}
              disabled={busy || (detail.status !== 'approved' && !detail.exemplar)}
              title={
                detail.status !== 'approved'
                  ? 'Approve the asset first — only approved assets shape the canon'
                  : 'Approved exemplars condition future generation'
              }
              className={`mb-4 inline-flex w-full items-center justify-center gap-1.5 rounded-[10px] border px-3 py-2 text-sm transition disabled:opacity-40 ${
                detail.exemplar
                  ? 'border-amber-400/40 bg-amber-400/10 text-amber-200 hover:bg-amber-400/15'
                  : 'border-white/10 text-text-dim hover:text-text'
              }`}
            >
              <StarIcon size={14} weight={detail.exemplar ? 'fill' : 'regular'} />
              {detail.exemplar ? 'Style exemplar (conditions new gens)' : 'Use as style exemplar'}
            </button>

            {detail.derivation && (
              <p className="mb-4 text-xs text-text-dim">
                <span className="text-text">Derivation:</span> {detail.derivation}
              </p>
            )}

            <label className="mb-3 grid gap-1.5">
              <span className="text-xs font-medium text-text-dim">Name</span>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={api.displayName(detail)}
                className="rounded-[10px] bg-surface/60 px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
              />
            </label>
            <label className="mb-3 grid gap-1.5">
              <span className="text-xs font-medium text-text-dim">Role</span>
              <input
                value={role}
                onChange={(e) => setRole(e.target.value)}
                placeholder="e.g. character, prop, tile"
                className="rounded-[10px] bg-surface/60 px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
              />
            </label>
            <label className="mb-3 grid gap-1.5">
              <span className="text-xs font-medium text-text-dim">Tags (comma-separated)</span>
              <input
                value={tags}
                onChange={(e) => setTags(e.target.value)}
                placeholder="hero, wip"
                className="rounded-[10px] bg-surface/60 px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-teal/40"
              />
            </label>
            <div className="mb-5 flex items-center gap-3">
              <button
                onClick={save}
                disabled={busy}
                className="inline-flex items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
              >
                {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <CheckIcon size={14} weight="bold" />}
                Save
              </button>
              {saved && <span className="text-xs text-teal-bright">Saved</span>}
              {error && <span className="text-xs text-rose-300">{error}</span>}
            </div>

            {collections.length > 0 ? (
              <div className="mb-5 flex flex-wrap items-center gap-2">
                <select
                  value={selectedCol}
                  onChange={(e) => {
                    setSelectedCol(e.target.value)
                    setAdded(null)
                  }}
                  className="rounded-[10px] bg-surface/60 px-2.5 py-2 text-sm text-text outline-none focus:ring-1 focus:ring-teal/40"
                >
                  <option value="">Add to collection…</option>
                  {collections.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
                <button
                  onClick={addTo}
                  disabled={!selectedCol}
                  className="rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-text-dim transition hover:text-text disabled:opacity-40"
                >
                  Add
                </button>
                {added && <span className="text-xs text-teal-bright">Added to {added}</span>}
              </div>
            ) : (
              <p className="mb-5 text-xs text-text-dim">No collections yet — create one in the Collections tab.</p>
            )}

            <div className="flex items-center gap-1.5 border-t border-white/8 pt-3 text-xs text-text-dim">
              <TreeStructureIcon size={14} />
              Lineage
            </div>

            {detail.base && (
              <div className="mt-3">
                <p className="mb-1.5 text-[11px] text-text-dim">Derived from</p>
                <button
                  onClick={() => onNavigate(detail.base!.id)}
                  className="flex w-full items-center gap-2 rounded-[10px] p-1.5 text-left transition hover:bg-white/5"
                >
                  <img src={detail.base.url} alt="" className="size-12 rounded-[8px] object-cover ring-1 ring-white/10" />
                  <span className="truncate text-xs text-text">{detail.base.role ?? detail.base.prompt ?? 'base'}</span>
                </button>
              </div>
            )}

            {detail.derivatives.length > 0 && (
              <div className="mt-3">
                <p className="mb-1.5 text-[11px] text-text-dim">{detail.derivatives.length} derivatives</p>
                <div className="grid grid-cols-4 gap-2">
                  {detail.derivatives.map((d) => (
                    <button
                      key={d.id}
                      onClick={() => onNavigate(d.id)}
                      title={d.derivation ?? ''}
                      className="overflow-hidden rounded-[8px] ring-1 ring-white/10 transition hover:ring-teal"
                    >
                      <img src={d.url} alt="" className="aspect-square w-full object-cover" />
                    </button>
                  ))}
                </div>
              </div>
            )}

            {!detail.base && detail.derivatives.length === 0 && (
              <p className="mt-3 text-xs text-text-dim">No lineage yet — derive from this asset to grow it.</p>
            )}

            <div className="mt-6 border-t border-white/8 pt-4">
              <CommentThread assetId={detail.id} />
            </div>

            <div className="mt-6 border-t border-white/8 pt-3">
              {confirming ? (
                <div className="flex items-center gap-2">
                  <button
                    onClick={remove}
                    disabled={deleting}
                    className="inline-flex items-center gap-1.5 rounded-[8px] bg-rose-500/90 px-3 py-1.5 text-sm font-medium text-white transition hover:bg-rose-500 disabled:opacity-50"
                  >
                    {deleting ? <SpinnerGapIcon size={14} className="animate-spin" /> : <TrashIcon size={14} />}
                    Confirm delete
                  </button>
                  <button onClick={() => setConfirming(false)} className="text-xs text-text-dim hover:text-text">
                    Cancel
                  </button>
                </div>
              ) : (
                <button
                  onClick={() => setConfirming(true)}
                  className="inline-flex items-center gap-1.5 text-sm text-rose-300 transition hover:text-rose-200"
                >
                  <TrashIcon size={14} />
                  Delete asset
                </button>
              )}
            </div>
          </div>
        )}
      </aside>
    </>
  )
}
