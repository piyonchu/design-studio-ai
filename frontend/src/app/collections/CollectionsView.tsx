import { useEffect, useState, type FormEvent } from 'react'
import {
  StackIcon,
  PlusIcon,
  SpinnerGapIcon,
  TrashIcon,
  ArrowLeftIcon,
  XIcon,
  PackageIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { formatApiError } from '../../lib/api'
import { ExportDialog } from '../export/ExportDialog'
import { Panel, PanelBody, PanelHeader, PanelIcon, PanelInset } from '../ui/Panel'
import { ErrorBanner } from '../ui/ErrorBanner'
import { AssetImage } from '../ui/AssetImage'

/**
 * Collections — asset packs. List view (cards + create) and a detail view
 * (the pack's assets, remove + delete). Assets are added from the inspector.
 */
export function CollectionsView({ projectId, vertical }: { projectId: string; vertical?: string }) {
  const [collections, setCollections] = useState<api.CollectionSummary[]>([])
  const [openId, setOpenId] = useState<string | null>(null)
  const [detail, setDetail] = useState<api.CollectionDetail | null>(null)
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [exporting, setExporting] = useState(false)
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
      .catch((e) => alive && setError(formatApiError(e, "Couldn't load this collection. Try again.")))
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
      setError(formatApiError(err, "Couldn't create the collection. Try a different name."))
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
      setError(formatApiError(err, "Couldn't remove the asset from this collection. Try again."))
    }
  }

  async function del(id: string) {
    try {
      await api.deleteCollection(id)
      setCollections((c) => c.filter((x) => x.id !== id))
      if (openId === id) setOpenId(null)
    } catch (err) {
      setError(formatApiError(err, "Couldn't delete the collection. Try again."))
    }
  }

  // ── Detail view ─────────────────────────────────────────────────────────────
  if (openId) {
    return (
      <Panel>
        <PanelHeader>
          <button
            onClick={() => setOpenId(null)}
            aria-label="Back to collections"
            className="icon-btn size-7"
          >
            <ArrowLeftIcon size={16} />
          </button>
          <p className="text-sm font-medium text-text">{detail?.name ?? 'Collection'}</p>
          <span className="text-sm text-text-dim">· {detail?.assets.length ?? 0}</span>
          <button
            onClick={() => setExporting(true)}
            disabled={!detail || detail.assets.length === 0}
            className="ml-auto inline-flex items-center gap-1.5 rounded-[8px] bg-teal px-3 py-1.5 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-40"
          >
            <PackageIcon size={14} weight="fill" /> Export
          </button>
          <button
            onClick={() => del(openId)}
            className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-rose-300 transition hover:text-rose-200"
          >
            <TrashIcon size={14} /> Delete pack
          </button>
        </PanelHeader>

        {exporting && detail && (
          <ExportDialog
            projectId={projectId}
            assetIds={detail.assets.map((a) => a.id)}
            title={detail.name}
            vertical={vertical}
            onClose={() => setExporting(false)}
          />
        )}
        {error && (
          <PanelInset>
            <ErrorBanner message={error} onDismiss={() => setError(null)} />
          </PanelInset>
        )}
        <PanelBody>
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
                  <AssetImage src={a.url} alt="" className="aspect-square w-full object-cover" />
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
        </PanelBody>
      </Panel>
    )
  }

  // ── List view ───────────────────────────────────────────────────────────────
  return (
    <Panel>
      <PanelHeader>
        <PanelIcon>
          <StackIcon size={15} weight="fill" />
        </PanelIcon>
        <p className="text-sm font-medium text-text">Collections</p>
        <span className="text-sm text-text-dim">· {collections.length}</span>
      </PanelHeader>

      <form onSubmit={create} className="border-b border-white/8 p-4">
        <div className="mx-auto flex max-w-2xl items-center gap-2 rounded-[12px] bg-surface-2/60 p-2">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Collection name (e.g. Hero walk cycle)"
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
        {error && (
          <p className="mx-auto mt-2 max-w-2xl">
            <ErrorBanner message={error} onDismiss={() => setError(null)} />
          </p>
        )}
      </form>

      <PanelBody>
        {collections.length === 0 ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">
            No collections yet. Name one above to group assets for export.
          </p>
        ) : (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {collections.map((c) => (
              <button
                key={c.id}
                onClick={() => setOpenId(c.id)}
                className="group overflow-hidden rounded-[12px] text-left ring-1 ring-white/10 transition hover:ring-teal/40"
              >
                <div className="flex aspect-[4/3] w-full items-center justify-center bg-gradient-to-br from-indigo/12 via-surface-2/80 to-teal/10">
                  {c.cover_asset_id ? (
                    <AssetImage
                      src={`/api/assets/${c.cover_asset_id}/file`}
                      alt=""
                      className="h-full w-full object-cover"
                    />
                  ) : (
                    <StackIcon size={28} className="text-indigo-bright/35" weight="duotone" />
                  )}
                </div>
                <div className="px-2.5 py-2">
                  <p className="truncate text-sm text-text">{c.name}</p>
                  <p className={`text-[11px] ${c.item_count > 0 ? 'text-teal-bright' : 'text-text-dim'}`}>
                    {c.item_count} {c.item_count === 1 ? 'asset' : 'assets'}
                  </p>
                </div>
              </button>
            ))}
          </div>
        )}
      </PanelBody>
    </Panel>
  )
}
