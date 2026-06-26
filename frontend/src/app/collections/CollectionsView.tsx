import { useEffect, useState, type FormEvent } from 'react'
import {
  StackIcon,
  PlusIcon,
  SpinnerGapIcon,
  TrashIcon,
  ArrowLeftIcon,
  XIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'

/**
 * Collections — asset packs. List view (cards + create) and a detail view
 * (the pack's assets, remove + delete). Assets are added from the inspector.
 */
export function CollectionsView({ projectId }: { projectId: string }) {
  const [collections, setCollections] = useState<api.CollectionSummary[]>([])
  const [openId, setOpenId] = useState<string | null>(null)
  const [detail, setDetail] = useState<api.CollectionDetail | null>(null)
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.listCollections(projectId).then(setCollections).catch(() => {})
  }, [projectId])

  useEffect(() => {
    if (!openId) {
      setDetail(null)
      return
    }
    let alive = true
    setDetail(null)
    api
      .getCollection(openId)
      .then((d) => alive && setDetail(d))
      .catch((e) => alive && setError(e instanceof ApiError ? e.message : 'Failed to load.'))
    return () => {
      alive = false
    }
  }, [openId])

  async function create(e: FormEvent) {
    e.preventDefault()
    const n = name.trim()
    if (!n || busy) return
    setBusy(true)
    setError(null)
    try {
      await api.createCollection(projectId, n)
      setName('')
      setCollections(await api.listCollections(projectId))
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Create failed.')
    } finally {
      setBusy(false)
    }
  }

  async function removeItem(assetId: string) {
    if (!openId) return
    try {
      await api.removeFromCollection(openId, assetId)
      setDetail((d) => (d ? { ...d, assets: d.assets.filter((a) => a.id !== assetId) } : d))
      setCollections((c) =>
        c.map((x) => (x.id === openId ? { ...x, item_count: x.item_count - 1 } : x)),
      )
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Remove failed.')
    }
  }

  async function del(id: string) {
    try {
      await api.deleteCollection(id)
      setCollections((c) => c.filter((x) => x.id !== id))
      if (openId === id) setOpenId(null)
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Delete failed.')
    }
  }

  // ── Detail view ─────────────────────────────────────────────────────────────
  if (openId) {
    return (
      <div className="glass flex min-h-0 flex-1 flex-col rounded-[16px]">
        <div className="flex items-center gap-2 border-b border-white/8 px-5 py-4">
          <button
            onClick={() => setOpenId(null)}
            aria-label="Back to collections"
            className="grid size-7 place-items-center rounded-[8px] text-text-dim transition hover:bg-white/5 hover:text-text"
          >
            <ArrowLeftIcon size={16} />
          </button>
          <p className="text-sm font-medium text-text">{detail?.name ?? 'Collection'}</p>
          <span className="text-sm text-text-dim">· {detail?.assets.length ?? 0}</span>
          <button
            onClick={() => del(openId)}
            className="ml-auto inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-rose-300 transition hover:text-rose-200"
          >
            <TrashIcon size={14} /> Delete pack
          </button>
        </div>
        {error && <p className="px-5 pt-3 text-xs text-rose-300">{error}</p>}
        <div className="min-h-0 flex-1 overflow-y-auto p-5">
          {!detail ? (
            <div className="grid place-items-center py-16 text-text-dim">
              <SpinnerGapIcon size={20} className="animate-spin" />
            </div>
          ) : detail.assets.length === 0 ? (
            <p className="px-1 py-16 text-center text-sm text-text-dim">
              Empty pack. Add assets from an asset's inspector.
            </p>
          ) : (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
              {detail.assets.map((a) => (
                <figure
                  key={a.id}
                  className="group relative overflow-hidden rounded-[12px] ring-1 ring-white/10"
                  title={a.derivation ?? a.prompt ?? a.role ?? ''}
                >
                  <img src={a.url} alt="" className="aspect-square w-full object-cover" />
                  <button
                    onClick={() => removeItem(a.id)}
                    aria-label="Remove from pack"
                    className="absolute right-1.5 top-1.5 grid size-6 place-items-center rounded-[6px] bg-black/55 text-white/90 opacity-0 backdrop-blur transition group-hover:opacity-100"
                  >
                    <XIcon size={13} />
                  </button>
                </figure>
              ))}
            </div>
          )}
        </div>
      </div>
    )
  }

  // ── List view ───────────────────────────────────────────────────────────────
  return (
    <div className="glass flex min-h-0 flex-1 flex-col rounded-[16px]">
      <div className="flex items-center gap-2 border-b border-white/8 px-5 py-4">
        <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
          <StackIcon size={15} weight="fill" />
        </span>
        <p className="text-sm font-medium text-text">Collections</p>
        <span className="text-sm text-text-dim">· {collections.length}</span>
      </div>

      <form onSubmit={create} className="border-b border-white/8 p-4">
        <div className="mx-auto flex max-w-2xl items-center gap-2 rounded-[12px] bg-surface-2/60 p-2">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="New pack name… (e.g. Hero walk cycle)"
            className="flex-1 bg-transparent px-2 text-sm text-text outline-none placeholder:text-text-dim"
          />
          <button
            type="submit"
            disabled={busy || !name.trim()}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
          >
            {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <PlusIcon size={14} weight="bold" />}
            Create
          </button>
        </div>
        {error && <p className="mx-auto mt-2 max-w-2xl text-xs text-rose-300">{error}</p>}
      </form>

      <div className="min-h-0 flex-1 overflow-y-auto p-5">
        {collections.length === 0 ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">
            No collections yet. Create a pack above.
          </p>
        ) : (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {collections.map((c) => (
              <button
                key={c.id}
                onClick={() => setOpenId(c.id)}
                className="group overflow-hidden rounded-[12px] text-left ring-1 ring-white/10 transition hover:ring-white/25"
              >
                <div className="aspect-[4/3] w-full bg-surface-2/60">
                  {c.cover_asset_id && (
                    <img
                      src={`/api/assets/${c.cover_asset_id}/file`}
                      alt=""
                      className="h-full w-full object-cover"
                    />
                  )}
                </div>
                <div className="px-2.5 py-2">
                  <p className="truncate text-sm text-text">{c.name}</p>
                  <p className="text-[11px] text-text-dim">
                    {c.item_count} {c.item_count === 1 ? 'asset' : 'assets'}
                  </p>
                </div>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
