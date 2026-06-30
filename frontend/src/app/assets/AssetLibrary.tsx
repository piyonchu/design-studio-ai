import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from 'react'
import {
  SparkleIcon,
  SpinnerGapIcon,
  ImageIcon,
  UploadSimpleIcon,
  CheckIcon,
  XIcon,
  MagnifyingGlassIcon,
  CheckSquareIcon,
  FlagIcon,
  StackPlusIcon,
  PackageIcon,
  MusicNotesIcon,
  WarningIcon,
  StarIcon,
  BookmarkSimpleIcon,
  MinusIcon,
  PlusIcon,
  CircleDashedIcon,
  SidebarSimpleIcon,
} from '@phosphor-icons/react'
import type { Icon } from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { formatApiError } from '../../lib/api'
import { AssetInspector } from './AssetInspector'
import { ExportDialog } from '../export/ExportDialog'
import { JobsBanner } from './JobsBanner'
import { FolderTree } from './FolderTree'
import { verticalConfig } from '../verticals'
import { ConfirmDialog } from '../ui/Dialog'
import { ErrorBanner } from '../ui/ErrorBanner'
import { AssetImage } from '../ui/AssetImage'
import { Panel, PanelBody, PanelHeader, PanelIcon, PanelInset, PanelToolbar, RailSection } from '../ui/Panel'

const STATUSES: api.AssetStatus[] = ['candidate', 'approved', 'needs_review', 'rejected']

// Status as visual language on the board: candidate = dashed amber, approved =
// solid teal, needs_review = rose (with a pulsing flag), rejected = dimmed.
const STATUS_RING: Record<api.AssetStatus, string> = {
  candidate: 'ring-1 ring-warning/45',
  approved: 'ring-2 ring-teal/70',
  needs_review: 'ring-2 ring-danger/55',
  rejected: 'ring-1 ring-white/10 opacity-50',
}
const STATUS_DOT: Record<api.AssetStatus, string> = {
  candidate: 'bg-warning',
  approved: 'bg-teal',
  needs_review: 'bg-danger',
  rejected: 'bg-white/30',
}
// Status as a labelled chip on every tile — colour PLUS icon + word, so review
// state survives for colour-blind/greyscale users (DESIGN.md "Status Must
// Survive Rule"). One chip, top-right, hides on hover to reveal the inspect btn.
const STATUS_FILTER_TONE: Record<api.AssetStatus, 'teal' | 'amber' | 'rose' | 'dim'> = {
  candidate: 'amber',
  approved: 'teal',
  needs_review: 'rose',
  rejected: 'dim',
}

const SOURCE_CHIP: Record<string, string> = {
  uploaded: 'bg-indigo/20 text-indigo-bright',
  seeded: 'bg-teal/20 text-teal-bright',
  derived: 'bg-warning/20 text-warning',
}
const STATUS_CHIP: Record<api.AssetStatus, { label: string; cls: string; Icon: Icon; pulse?: boolean }> = {
  candidate: { label: 'Candidate', cls: 'bg-warning/90 text-bg', Icon: CircleDashedIcon },
  approved: { label: 'Approved', cls: 'bg-teal/90 text-bg', Icon: CheckIcon },
  needs_review: { label: 'Review', cls: 'bg-danger/90 text-white', Icon: FlagIcon, pulse: true },
  rejected: { label: 'Rejected', cls: 'bg-black/70 text-text-muted', Icon: XIcon },
}

// Rail building blocks — module-scoped so they keep a stable component identity
// (defining them inside AssetLibrary remounted the whole rail on every keystroke).
function FilterChip({
  active,
  count,
  onClick,
  children,
  activeTone = 'teal',
}: {
  active: boolean
  count?: number
  onClick: () => void
  children: ReactNode
  activeTone?: 'teal' | 'amber' | 'rose' | 'dim'
}) {
  const activeCls = {
    teal: 'bg-teal/15 text-teal-bright',
    amber: 'bg-warning/15 text-warning',
    rose: 'bg-danger/15 text-danger',
    dim: 'bg-white/10 text-text-muted',
  }[activeTone]
  return (
    <button
      onClick={onClick}
      className={`flex w-full items-center gap-2 rounded-[8px] px-2.5 py-1.5 text-left text-xs capitalize transition ${
        active ? activeCls : 'text-text-dim hover:bg-white/5 hover:text-text'
      }`}
    >
      <span className="flex-1 truncate">{children}</span>
      {count != null && <span className="text-[10px] tabular-nums text-text-dim">{count}</span>}
    </button>
  )
}

/**
 * The smart asset board — generate / upload / derive plus a filter rail
 * (role · status · source · collection), free-text search, status visual
 * language, and multi-select batch actions (approve / add-to-collection).
 * Click a tile to pick a derivation base; toggle Select for batch mode.
 */
export function AssetLibrary({
  projectId,
  vertical,
  canApprove = true,
}: {
  projectId: string
  vertical?: string
  canApprove?: boolean
}) {
  const PRESETS = verticalConfig(vertical).derivePresets
  const [assets, setAssets] = useState<api.Asset[]>([])
  const [collections, setCollections] = useState<api.CollectionSummary[]>([])
  const [prompt, setPrompt] = useState('')
  const [genMode, setGenMode] = useState<'image' | 'audio'>('image')
  // How many to generate per request (backend clamps 1–4).
  const [count, setCount] = useState(1)
  const [baseId, setBaseId] = useState<string | null>(null)
  const [instruction, setInstruction] = useState('')
  const [busy, setBusy] = useState(false)
  const [inspectId, setInspectId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [boardLoading, setBoardLoading] = useState(true)
  const [boardError, setBoardError] = useState<string | null>(null)
  const [reloadTick, setReloadTick] = useState(0)

  // Filters
  const [query, setQuery] = useState('')
  // Server-side smart-search hits (null = no active search → show all assets).
  const [searchHits, setSearchHits] = useState<api.Asset[] | null>(null)
  // Pre-generate dedup nudge: existing assets close to the typed prompt.
  const [dupHits, setDupHits] = useState<api.ScoredAsset[]>([])
  const [roles, setRoles] = useState<Set<string>>(new Set())
  const [statuses, setStatuses] = useState<Set<api.AssetStatus>>(new Set())
  const [sources, setSources] = useState<Set<string>>(new Set())
  const [collectionId, setCollectionId] = useState<string | null>(null)
  // assetId-sets for the active collection filter, fetched lazily + cached.
  const [collMembers, setCollMembers] = useState<Record<string, Set<string>>>({})
  // Folder tree + the active folder scope (null = all assets, 'root' = unfiled).
  const [folders, setFolders] = useState<api.FolderNode[]>([])
  const [folderSel, setFolderSel] = useState<string | null>(null)
  // QA gate: show only off-style candidates (style_fit below the threshold).
  const [offStyle, setOffStyle] = useState(false)
  // A3: the library is the hero; the generate bar is collapsed behind "+ New".
  const [showGen, setShowGen] = useState(false)
  // The filter rail can collapse to reclaim board width (and is the first step
  // toward a mobile drawer).
  const [railOpen, setRailOpen] = useState(true)

  // Multi-select
  const [selecting, setSelecting] = useState(false)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [batchCol, setBatchCol] = useState('')
  const [exportIds, setExportIds] = useState<string[] | null>(null)
  // A confirm step for the costly / canon-altering bulk actions (derive-all,
  // batch-approve) — they used to fire many irreversible ops on one click.
  const [confirm, setConfirm] = useState<{
    title: string
    body: ReactNode
    confirmLabel: string
    tone?: 'primary' | 'danger'
    run: () => void
  } | null>(null)

  const [recipes, setRecipes] = useState<api.Recipe[]>([])
  // Keyset pagination + whole-project facet counts (so the rail stays accurate
  // even though only a page of assets is loaded).
  const [nextCursor, setNextCursor] = useState<string | null>(null)
  const [loadingMore, setLoadingMore] = useState(false)
  const [facets, setFacets] = useState<api.AssetFacets | null>(null)
  const [facetsBump, setFacetsBump] = useState(0)
  const bumpFacets = () => setFacetsBump((n) => n + 1)
  const PAGE = 50

  const reloadFolders = () => api.listFolders(projectId).then(setFolders).catch(() => {})

  useEffect(() => {
    api.listCollections(projectId).then(setCollections).catch(() => {})
    api.listRecipes(projectId).then(setRecipes).catch(() => {})
    reloadFolders()
  }, [projectId])

  // Filter-rail counts over the whole project; refetched after mutations.
  useEffect(() => {
    api.getAssetFacets(projectId).then(setFacets).catch(() => {})
  }, [projectId, facetsBump])

  // First page — reloads whenever a server-side filter changes. Server does the
  // filtering + ordering; we just render the page.
  useEffect(() => {
    let alive = true
    setBoardLoading(true)
    setBoardError(null)
    api
      .listAssets(projectId, {
        limit: PAGE,
        status: [...statuses],
        role: [...roles],
        source: [...sources],
        collection: collectionId,
        folder: folderSel,
        offStyle,
      })
      .then((page) => {
        if (!alive) return
        setAssets(page.items)
        setNextCursor(page.next_cursor)
      })
      .catch((err) => {
        if (alive) setBoardError(formatApiError(err, "Couldn't load the asset board. Check your connection and retry."))
      })
      .finally(() => {
        if (alive) setBoardLoading(false)
      })
    return () => {
      alive = false
    }
  }, [projectId, statuses, roles, sources, collectionId, folderSel, offStyle, reloadTick])

  function reloadBoard() {
    setReloadTick((n) => n + 1)
  }

  async function loadMore() {
    if (!nextCursor || loadingMore) return
    setLoadingMore(true)
    try {
      const page = await api.listAssets(projectId, {
        limit: PAGE,
        cursor: nextCursor,
        status: [...statuses],
        role: [...roles],
        source: [...sources],
        collection: collectionId,
        folder: folderSel,
        offStyle,
      })
      setAssets((a) => [...a, ...page.items])
      setNextCursor(page.next_cursor)
    } catch (err) {
      setError(formatApiError(err, "Couldn't load more assets. Try again."))
    } finally {
      setLoadingMore(false)
    }
  }

  async function saveRecipe() {
    const ins = instruction.trim()
    if (!ins || busy) return
    const name = ins.split(/\s+/).slice(0, 4).join(' ').slice(0, 40)
    try {
      const r = await api.createRecipe(projectId, name, ins)
      setRecipes((rs) => [r, ...rs])
    } catch (err) {
      setError(formatApiError(err, "Couldn't save the recipe. Try again."))
    }
  }

  async function removeRecipe(id: string) {
    try {
      await api.deleteRecipe(id)
      setRecipes((rs) => rs.filter((r) => r.id !== id))
    } catch {
      /* ignore */
    }
  }

  // Debounced smart search — server-side semantic/keyword ranking. Empty query
  // clears hits and falls back to the full (client-filtered) library.
  useEffect(() => {
    const q = query.trim()
    if (!q) {
      setSearchHits(null)
      return
    }
    const t = setTimeout(() => {
      api.searchAssets(projectId, q).then(setSearchHits).catch(() => setSearchHits(null))
    }, 300)
    return () => clearTimeout(t)
  }, [query, projectId, assets])

  // Debounced pre-generate dedup nudge (image mode only).
  useEffect(() => {
    const p = prompt.trim()
    if (genMode !== 'image' || baseId || p.length < 4) {
      setDupHits([])
      return
    }
    const t = setTimeout(() => {
      api.similarCheck(projectId, p).then(setDupHits).catch(() => setDupHits([]))
    }, 500)
    return () => clearTimeout(t)
  }, [prompt, genMode, baseId, projectId])

  // Lazily load member ids when a collection filter is selected.
  useEffect(() => {
    if (!collectionId || collMembers[collectionId]) return
    api
      .getCollection(collectionId)
      .then((c) => setCollMembers((m) => ({ ...m, [collectionId]: new Set(c.assets.map((a) => a.id)) })))
      .catch(() => {})
  }, [collectionId, collMembers])

  // Rail options/counts come from the whole-project facets, not the loaded page.
  const roleOptions = useMemo(
    () => (facets?.role ?? []).map((f) => [f.value, f.count] as [string, number]),
    [facets],
  )
  const sourceOptions = useMemo(
    () => (facets?.source ?? []).map((f) => [f.value, f.count] as [string, number]),
    [facets],
  )
  const statusCounts = useMemo(() => {
    const m = new Map<api.AssetStatus, number>()
    for (const f of facets?.status ?? []) m.set(f.value as api.AssetStatus, f.count)
    return m
  }, [facets])

  // What to render. Browse mode: `assets` is already server-filtered + paged.
  // Search mode: narrow the bounded ranked hits client-side by the rail.
  const displayed = useMemo(() => {
    if (searchHits == null) return assets
    const members = collectionId ? collMembers[collectionId] : null
    return searchHits.filter((a) => {
      if (roles.size && (!a.role || !roles.has(a.role))) return false
      if (statuses.size && !statuses.has(a.status)) return false
      if (sources.size && !sources.has(a.source_kind)) return false
      if (collectionId && !(members?.has(a.id) ?? false)) return false
      if (folderSel === 'root' && a.folder_id !== null) return false
      if (folderSel && folderSel !== 'root' && a.folder_id !== folderSel) return false
      if (offStyle && !(a.style_fit != null && a.style_fit < api.STYLE_FIT_THRESHOLD)) return false
      return true
    })
  }, [assets, searchHits, roles, statuses, sources, collectionId, collMembers, folderSel, offStyle])

  const activeFilters =
    roles.size + statuses.size + sources.size + (collectionId ? 1 : 0) + (query.trim() ? 1 : 0) + (folderSel ? 1 : 0) + (offStyle ? 1 : 0)

  function toggle<T>(set: Set<T>, value: T): Set<T> {
    const next = new Set(set)
    next.has(value) ? next.delete(value) : next.add(value)
    return next
  }

  function clearFilters() {
    setQuery('')
    setRoles(new Set())
    setStatuses(new Set())
    setSources(new Set())
    setCollectionId(null)
    setFolderSel(null)
    setOffStyle(false)
  }

  function genError(err: unknown) {
    setError(formatApiError(err, "Couldn't complete that request. Try again."))
  }

  // Poll a generation job to completion (non-blocking), then refresh the board.
  async function watchJob(jobId: string) {
    for (let i = 0; i < 80; i++) {
      let job: api.Job
      try {
        job = await api.getJob(jobId)
      } catch {
        return
      }
      if (job.status === 'succeeded') {
        api
          .listAssets(projectId, {
            limit: PAGE,
            status: [...statuses],
            role: [...roles],
            source: [...sources],
            collection: collectionId,
            folder: folderSel,
          })
          .then((page) => {
            setAssets(page.items)
            setNextCursor(page.next_cursor)
          })
          .catch(() => {})
        bumpFacets()
        reloadFolders()
        return
      }
      if (job.status === 'failed') {
        setError(job.error || "Generation didn't finish. Check the board and try again.")
        return
      }
      await new Promise((r) => setTimeout(r, 700))
    }
  }

  async function generate(e: FormEvent) {
    e.preventDefault()
    const p = prompt.trim()
    if (!p || busy) return
    setBusy(true)
    setError(null)
    try {
      if (genMode === 'audio') {
        // Audio stays synchronous (no async job kind yet).
        const created = await api.generateAudio(projectId, p, count)
        setAssets((a) => [...created, ...a])
        bumpFacets()
      } else {
        // Image generation runs as a background job: enqueue, then watch it
        // finish and refresh the board (the JobsBanner shows progress).
        const job = await api.enqueueGenerate(projectId, p, count)
        watchJob(job.id)
      }
      setPrompt('')
      setDupHits([])
    } catch (err) {
      genError(err)
    } finally {
      setBusy(false)
    }
  }

  async function upload(file: File) {
    if (!file.type.startsWith('image/')) {
      setError('Choose an image file — PNG, JPEG, or WebP.')
      return
    }
    const maxBytes = 20 * 1024 * 1024
    if (file.size > maxBytes) {
      setError('Images must be 20 MB or smaller.')
      return
    }
    if (busy) return
    setBusy(true)
    setError(null)
    try {
      const created = await api.uploadAsset(projectId, file, 'base')
      setAssets((a) => [created, ...a])
      bumpFacets()
    } catch (err) {
      setError(formatApiError(err, "Couldn't upload that image. Try again."))
    } finally {
      setBusy(false)
    }
  }

  function pickFile() {
    const inp = document.createElement('input')
    inp.type = 'file'
    inp.accept = 'image/*'
    inp.onchange = () => {
      const f = inp.files?.[0]
      if (f) upload(f)
    }
    inp.click()
  }

  async function derive() {
    const ins = instruction.trim()
    if (!ins || !baseId || busy) return
    setBusy(true)
    setError(null)
    try {
      const created = await api.deriveAssets(projectId, baseId, ins, 1)
      setAssets((a) => [...created, ...a])
      bumpFacets()
    } catch (err) {
      genError(err)
    } finally {
      setBusy(false)
    }
  }

  // "Make the other 200": derive one of every preset from the base in a single
  // action — a whole consistent set (walk/action/variant/... or manhwa's
  // expression/pose/...). Each is its own gen; we collect what succeeds.
  async function deriveAll() {
    if (!baseId || busy) return
    setBusy(true)
    setError(null)
    // Collect per-preset outcomes so a mid-loop failure reports a partial
    // summary instead of each error clobbering the last.
    let ok = 0
    const failed: string[] = []
    for (const p of PRESETS) {
      try {
        const created = await api.deriveAssets(projectId, baseId, p.text, 1)
        setAssets((a) => [...created, ...a])
        ok++
      } catch {
        failed.push(p.label)
      }
    }
    bumpFacets()
    setBusy(false)
    if (failed.length) {
      setError(`Derived ${ok} of ${PRESETS.length}. These presets failed: ${failed.join(', ')}.`)
    }
  }

  // Move an asset into a folder (or to root with null). Drives drag-onto-folder
  // and the inspector's folder picker. Drops the tile from view if it no longer
  // matches the active folder scope, and refreshes the tree counts.
  async function moveAssetToFolder(assetId: string, folderId: string | null) {
    try {
      const updated = await api.moveAsset(assetId, folderId)
      const matches =
        folderSel === null ||
        (folderSel === 'root' && updated.folder_id === null) ||
        folderSel === updated.folder_id
      setAssets((a) =>
        matches ? a.map((x) => (x.id === assetId ? updated : x)) : a.filter((x) => x.id !== assetId),
      )
      reloadFolders()
    } catch (err) {
      genError(err)
    }
  }

  async function review(id: string, status: api.AssetStatus) {
    try {
      const updated = await api.setAssetStatus(id, status)
      setAssets((a) => a.map((x) => (x.id === id ? updated : x)))
      bumpFacets()
    } catch (err) {
      setError(formatApiError(err, "Couldn't update that asset. Try again."))
    }
  }

  // ── Multi-select ──────────────────────────────────────────────────────────
  function toggleSelectMode() {
    setSelecting((s) => !s)
    setSelected(new Set())
    setBaseId(null)
  }

  function onTileClick(a: api.Asset) {
    if (selecting) {
      setSelected((s) => toggle(s, a.id))
    } else {
      setBaseId(a.id === baseId ? null : a.id)
    }
  }

  async function batchStatus(status: api.AssetStatus) {
    const ids = [...selected]
    if (!ids.length || busy) return
    setBusy(true)
    setError(null)
    try {
      const updated = await Promise.all(ids.map((id) => api.setAssetStatus(id, status)))
      const byId = new Map(updated.map((u) => [u.id, u]))
      setAssets((a) => a.map((x) => byId.get(x.id) ?? x))
      setSelected(new Set())
      bumpFacets()
    } catch (err) {
      setError(formatApiError(err, "Couldn't update the selected assets. Try again."))
    } finally {
      setBusy(false)
    }
  }

  async function batchAddToCollection() {
    const ids = [...selected]
    if (!ids.length || !batchCol || busy) return
    setBusy(true)
    setError(null)
    try {
      await api.addToCollection(batchCol, ids)
      // Invalidate the cached membership so the collection filter re-fetches.
      setCollMembers((m) => {
        const next = { ...m }
        delete next[batchCol]
        return next
      })
      setCollections((cs) => cs.map((c) => (c.id === batchCol ? { ...c, item_count: c.item_count + ids.length } : c)))
      setBatchCol('')
      setSelected(new Set())
    } catch (err) {
      setError(formatApiError(err, "Couldn't add to the collection. Try again."))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Panel layout="split">
      {/* Filter rail (collapsible — reclaims board width) */}
      {railOpen ? (
      <aside className="flex min-h-0 w-56 shrink-0 flex-col border-r border-white/8">
        <PanelHeader size="compact">
          <div className="flex flex-1 items-center gap-2 rounded-[10px] bg-surface-2/60 px-2.5 py-2 transition focus-within:ring-1 focus-within:ring-teal/40">
            <MagnifyingGlassIcon size={14} className="text-text-dim" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search assets…"
              aria-label="Search assets"
              className="w-full bg-transparent text-xs text-text outline-none placeholder:text-text-dim"
            />
          </div>
          <button
            onClick={() => setRailOpen(false)}
            aria-label="Collapse filters"
            title="Collapse filters"
            className="icon-btn size-8 shrink-0 focus-visible:ring-2 focus-visible:ring-indigo/40"
          >
            <SidebarSimpleIcon size={16} />
          </button>
        </PanelHeader>

        <PanelBody density="dense">
          <FolderTree
            projectId={projectId}
            folders={folders}
            selected={folderSel}
            onSelect={setFolderSel}
            onChanged={reloadFolders}
            onMoveAsset={moveAssetToFolder}
          />

          <button
            onClick={() => setOffStyle((v) => !v)}
            title="Show only candidates that don't match your approved style"
            className={`mb-3 flex w-full items-center gap-2 rounded-[10px] border px-3 py-2 text-xs font-medium transition ${
              offStyle
                ? 'border-warning/40 bg-warning/10 text-warning'
                : 'border-white/10 text-text-dim hover:text-text'
            }`}
          >
            <WarningIcon size={14} weight="fill" className={offStyle ? 'text-warning' : ''} />
            Needs attention
          </button>

          <RailSection title="Status">
            {STATUSES.map((s) => (
              <FilterChip
                key={s}
                active={statuses.has(s)}
                count={statusCounts.get(s) ?? 0}
                activeTone={STATUS_FILTER_TONE[s]}
                onClick={() => setStatuses((set) => toggle(set, s))}
              >
                <span className="inline-flex items-center gap-1.5">
                  <span className={`size-2 rounded-full ${STATUS_DOT[s]}`} />
                  {s.replace('_', ' ')}
                </span>
              </FilterChip>
            ))}
          </RailSection>

          {roleOptions.length > 0 && (
            <RailSection title="Role">
              {roleOptions.map(([r, n]) => (
                <FilterChip key={r} active={roles.has(r)} count={n} onClick={() => setRoles((set) => toggle(set, r))}>
                  {r}
                </FilterChip>
              ))}
            </RailSection>
          )}

          <RailSection title="Source">
            {sourceOptions.map(([s, n]) => (
              <FilterChip key={s} active={sources.has(s)} count={n} onClick={() => setSources((set) => toggle(set, s))}>
                {s}
              </FilterChip>
            ))}
          </RailSection>

          {collections.length > 0 && (
            <RailSection title="Collection">
              {collections.map((c) => (
                <FilterChip
                  key={c.id}
                  active={collectionId === c.id}
                  count={c.item_count}
                  onClick={() => setCollectionId((id) => (id === c.id ? null : c.id))}
                >
                  {c.name}
                </FilterChip>
              ))}
            </RailSection>
          )}
        </PanelBody>

        {activeFilters > 0 && (
          <button
            onClick={clearFilters}
            className="border-t border-white/8 px-3 py-2.5 text-left text-xs text-text-dim transition hover:text-text"
          >
            Clear {activeFilters} filter{activeFilters > 1 ? 's' : ''}
          </button>
        )}
      </aside>
      ) : (
        <aside className="flex min-h-0 w-12 shrink-0 flex-col items-center gap-2 border-r border-white/8 py-3">
          <button
            onClick={() => setRailOpen(true)}
            aria-label="Show filters"
            title="Show filters"
            className="relative grid size-9 place-items-center rounded-[10px] text-text-dim transition hover:bg-white/5 hover:text-text"
          >
            <SidebarSimpleIcon size={18} />
            {activeFilters > 0 && (
              <span className="absolute -right-0.5 -top-0.5 grid size-4 place-items-center rounded-full bg-teal text-[9px] font-bold text-bg">
                {activeFilters}
              </span>
            )}
          </button>
        </aside>
      )}

      {/* Main column */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <PanelHeader>
          <PanelIcon>
            <ImageIcon size={15} weight="fill" />
          </PanelIcon>
          <p className="text-sm font-medium text-text">Asset Board</p>
          <span className="text-sm text-text-dim">
            · {displayed.length}
            {searchHits == null && nextCursor && '+'}
          </span>
          <button
            onClick={() => {
              setShowGen((v) => !v)
              setBaseId(null)
            }}
            className={`ml-auto inline-flex items-center gap-1.5 rounded-[8px] px-3 py-1.5 text-sm font-semibold transition ${
              showGen ? 'bg-teal/15 text-teal-bright' : 'bg-teal text-bg active:translate-y-px'
            }`}
          >
            <SparkleIcon size={14} weight="fill" />
            New asset
          </button>
          <button
            onClick={toggleSelectMode}
            className={`inline-flex items-center gap-1.5 rounded-[8px] border px-3 py-1.5 text-sm transition ${
              selecting ? 'border-teal/40 bg-teal/10 text-teal-bright' : 'border-white/10 text-text-dim hover:text-text'
            }`}
          >
            <CheckSquareIcon size={14} weight={selecting ? 'fill' : 'regular'} />
            Select
          </button>
          <button
            onClick={pickFile}
            disabled={busy}
            className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-text-dim transition hover:text-text disabled:opacity-50"
          >
            <UploadSimpleIcon size={14} />
            Upload base
          </button>
        </PanelHeader>

        {/* Batch toolbar (select mode) — else generate / derive bar */}
        {selecting ? (
          <PanelToolbar>
            <span className="text-sm text-text">{selected.size} selected</span>
            <button
              onClick={() =>
                setConfirm({
                  title: `Approve ${selected.size} ${selected.size === 1 ? 'asset' : 'assets'}?`,
                  body: (
                    <>
                      Approved assets enter the project canon and condition future generations.
                      You can still re-review each one in the Review tab.
                    </>
                  ),
                  confirmLabel: `Approve ${selected.size}`,
                  run: () => batchStatus('approved'),
                })
              }
              disabled={!selected.size || busy || !canApprove}
              title={canApprove ? undefined : 'Only a reviewer or owner can approve'}
              className="inline-flex items-center gap-1.5 rounded-[8px] bg-teal px-3 py-1.5 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-40"
            >
              <CheckIcon size={14} weight="bold" />
              Approve
            </button>
            <button
              onClick={() => batchStatus('rejected')}
              disabled={!selected.size || busy}
              className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-danger transition hover:bg-white/5 disabled:opacity-40"
            >
              <XIcon size={14} weight="bold" />
              Reject
            </button>
            {collections.length > 0 && (
              <div className="flex items-center gap-1.5">
                <select
                  value={batchCol}
                  onChange={(e) => setBatchCol(e.target.value)}
                  aria-label="Add selected assets to collection"
                  className="rounded-[8px] bg-surface-2/60 px-2.5 py-1.5 text-sm text-text outline-none focus:ring-1 focus:ring-teal/40"
                >
                  <option value="">Add to collection…</option>
                  {collections.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
                <button
                  onClick={batchAddToCollection}
                  disabled={!selected.size || !batchCol || busy}
                  className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-text-dim transition hover:text-text disabled:opacity-40"
                >
                  <StackPlusIcon size={14} />
                  Add
                </button>
              </div>
            )}
            <button
              onClick={() => selected.size && setExportIds([...selected])}
              disabled={!selected.size || busy}
              className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-text-dim transition hover:text-text disabled:opacity-40"
            >
              <PackageIcon size={14} />
              Export
            </button>
            {selected.size > 0 && (
              <button onClick={() => setSelected(new Set())} className="ml-auto text-xs text-text-dim hover:text-text">
                Clear selection
              </button>
            )}
          </PanelToolbar>
        ) : baseId ? (
          <div className="border-b border-white/8 p-4">
            <div className="mx-auto max-w-2xl">
              <div className="mb-2 flex items-center gap-2 text-xs text-text-dim">
                <span>Deriving from selected base — pick a preset or write an instruction</span>
                <button
                  onClick={() =>
                    setConfirm({
                      title: `Derive a full set of ${PRESETS.length}?`,
                      body: (
                        <>
                          This generates {PRESETS.length} variants from the selected base in one
                          step and uses your shared generation credit. Each lands on the board as a
                          candidate for review.
                        </>
                      ),
                      confirmLabel: `Derive ${PRESETS.length}`,
                      run: deriveAll,
                    })
                  }
                  disabled={busy}
                  title="Derive one of every preset — a whole consistent set"
                  className="ml-auto inline-flex items-center gap-1.5 rounded-[8px] border border-teal/30 bg-teal/10 px-2.5 py-1 text-teal-bright transition hover:bg-teal/15 disabled:opacity-50"
                >
                  {busy ? <SpinnerGapIcon size={12} className="animate-spin" /> : <SparkleIcon size={12} weight="fill" />}
                  Derive all {PRESETS.length}
                </button>
                <button onClick={() => setBaseId(null)} className="text-text-dim hover:text-text">
                  Clear
                </button>
              </div>
              <div className="mb-2 flex flex-wrap gap-1.5">
                {PRESETS.map((p) => (
                  <button
                    key={p.id}
                    onClick={() => setInstruction(p.text)}
                    className="rounded-[8px] border border-white/10 px-2.5 py-1 text-xs text-text-dim transition hover:text-text"
                  >
                    {p.label}
                  </button>
                ))}
              </div>
              {recipes.length > 0 && (
                <div className="mb-2 flex flex-wrap items-center gap-1.5">
                  <span className="text-[10px] font-semibold uppercase tracking-wider text-text-dim">Recipes</span>
                  {recipes.map((r) => (
                    <span
                      key={r.id}
                      className="group inline-flex items-center gap-1 rounded-[8px] border border-teal/25 bg-teal/8 py-1 pl-2.5 pr-1 text-xs text-teal-bright"
                    >
                      <button onClick={() => setInstruction(r.instruction)} title={r.instruction}>
                        {r.name}
                      </button>
                      <button
                        onClick={() => removeRecipe(r.id)}
                        aria-label="Delete recipe"
                        className="text-text-dim opacity-0 transition hover:text-danger group-hover:opacity-100"
                      >
                        <XIcon size={11} />
                      </button>
                    </span>
                  ))}
                </div>
              )}
              <div className="flex items-center gap-2 rounded-[12px] bg-surface-2/60 p-2 transition focus-within:ring-1 focus-within:ring-teal/40">
                <input
                  value={instruction}
                  onChange={(e) => setInstruction(e.target.value)}
                  placeholder="Derivation instruction…"
                  aria-label="Derivation instruction"
                  className="flex-1 bg-transparent px-2 text-sm text-text outline-none placeholder:text-text-dim"
                />
                <button
                  onClick={saveRecipe}
                  disabled={!instruction.trim()}
                  title="Save this instruction as a reusable recipe"
                  className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] border border-white/10 px-2.5 py-2 text-xs text-text-dim transition hover:text-text disabled:opacity-40"
                >
                  <BookmarkSimpleIcon size={14} />
                  Save
                </button>
                <button
                  onClick={derive}
                  disabled={busy || !instruction.trim()}
                  className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
                >
                  {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <SparkleIcon size={14} weight="fill" />}
                  Derive
                </button>
              </div>
            </div>
          </div>
        ) : showGen ? (
          <form onSubmit={generate} className="border-b border-white/8 p-4">
            <div className="mx-auto flex max-w-2xl items-center gap-2 rounded-[12px] bg-surface-2/60 p-2 transition focus-within:ring-1 focus-within:ring-teal/40">
              <div className="flex shrink-0 items-center rounded-[8px] bg-surface/60 p-0.5">
                {(['image', 'audio'] as const).map((m) => (
                  <button
                    key={m}
                    type="button"
                    onClick={() => setGenMode(m)}
                    aria-label={`Generate ${m}`}
                    className={`grid size-7 place-items-center rounded-[6px] transition ${
                      genMode === m ? 'bg-teal text-bg' : 'text-text-dim hover:text-text'
                    }`}
                  >
                    {m === 'image' ? <ImageIcon size={14} weight="fill" /> : <MusicNotesIcon size={14} weight="fill" />}
                  </button>
                ))}
              </div>
              <input
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder={
                  genMode === 'audio'
                    ? 'Describe a sound to generate… (e.g. sword clang, ambient loop)'
                    : 'Describe an asset to generate… (or click a tile to derive)'
                }
                aria-label={genMode === 'audio' ? 'Describe a sound to generate' : 'Describe an asset to generate'}
                className="flex-1 bg-transparent px-2 text-sm text-text outline-none placeholder:text-text-dim"
              />
              <div
                className="flex shrink-0 items-center gap-0.5 rounded-[8px] bg-surface/60 p-0.5"
                title="How many to generate (max 4)"
              >
                <button
                  type="button"
                  onClick={() => setCount((c) => Math.max(1, c - 1))}
                  disabled={count <= 1}
                  aria-label="Fewer"
                  className="grid size-7 place-items-center rounded-[6px] text-text-dim transition hover:text-text disabled:opacity-30"
                >
                  <MinusIcon size={13} weight="bold" />
                </button>
                <span className="w-5 text-center text-sm font-semibold tabular-nums text-text">{count}</span>
                <button
                  type="button"
                  onClick={() => setCount((c) => Math.min(4, c + 1))}
                  disabled={count >= 4}
                  aria-label="More"
                  className="grid size-7 place-items-center rounded-[6px] text-text-dim transition hover:text-text disabled:opacity-30"
                >
                  <PlusIcon size={13} weight="bold" />
                </button>
              </div>
              <button
                type="submit"
                disabled={busy || !prompt.trim()}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] bg-teal px-3.5 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
              >
                {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <SparkleIcon size={14} weight="fill" />}
                Generate
              </button>
            </div>
          </form>
        ) : null}

        {error && (
          <PanelInset>
            <ErrorBanner message={error} onDismiss={() => setError(null)} />
          </PanelInset>
        )}

        {boardError && (
          <PanelInset>
            <ErrorBanner
              message={boardError}
              onRetry={reloadBoard}
              onDismiss={() => setBoardError(null)}
            />
          </PanelInset>
        )}

        {!selecting && !baseId && dupHits.length > 0 && (
          <PanelInset>
          <div className="flex items-center gap-3 rounded-[10px] border border-warning/25 bg-warning/8 px-3 py-2">
            <WarningIcon size={16} weight="fill" className="shrink-0 text-warning" />
            <p className="shrink-0 text-xs text-warning">
              {dupHits.length} similar asset{dupHits.length > 1 ? 's' : ''} already exist
            </p>
            <div className="flex flex-1 items-center gap-1.5 overflow-x-auto">
              {dupHits.map((s) => (
                <button
                  key={s.id}
                  onClick={() => setInspectId(s.id)}
                  title={`${s.prompt ?? s.role ?? ''} · ${Math.round(s.score * 100)}% match`}
                  className="size-9 shrink-0 overflow-hidden rounded-[7px] ring-1 ring-white/15 transition hover:ring-teal"
                >
                  <AssetImage src={s.url} alt="" className="size-full object-cover" />
                </button>
              ))}
            </div>
            <button
              onClick={() => setDupHits([])}
              aria-label="Dismiss"
              className="shrink-0 text-text-dim hover:text-text"
            >
              <XIcon size={14} />
            </button>
          </div>
          </PanelInset>
        )}

        <PanelInset>
          <JobsBanner projectId={projectId} />
        </PanelInset>

        <PanelBody className="pt-3">
          {boardLoading && displayed.length === 0 ? (
            <div className="grid place-items-center py-16 text-text-dim">
              <SpinnerGapIcon size={20} className="animate-spin" />
            </div>
          ) : displayed.length === 0 ? (
            <p className="px-1 py-16 text-center text-sm text-text-dim">
              {activeFilters > 0
                ? 'No assets match these filters. Clear filters or try a different search.'
                : 'No assets on the board yet. Use New asset to upload a reference or generate your first one.'}
            </p>
          ) : (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
              {displayed.map((a) => {
                const isSel = selected.has(a.id)
                const isBase = !selecting && a.id === baseId
                // Selection uses indigo (multi-select / derive base) — distinct from
                // status rings (approved=teal, candidate=warning, needs_review=danger).
                const ring = isSel || isBase
                  ? 'ring-2 ring-indigo-bright/80'
                  : STATUS_RING[a.status]
                return (
                  <figure
                    key={a.id}
                    role="button"
                    tabIndex={0}
                    aria-pressed={isSel || isBase}
                    aria-label={`${api.displayName(a)} — ${STATUS_CHIP[a.status].label}. ${
                      selecting
                        ? isSel
                          ? 'Selected'
                          : 'Select'
                        : isBase
                          ? 'Derivation base'
                          : 'Set as derivation base'
                    }`}
                    onClick={() => onTileClick(a)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault()
                        onTileClick(a)
                      }
                    }}
                    draggable={!selecting}
                    onDragStart={(e) => {
                      e.dataTransfer.setData('text/asset-id', a.id)
                      e.dataTransfer.effectAllowed = 'move'
                    }}
                    className={`group relative cursor-pointer overflow-hidden rounded-[12px] transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-bright/80 ${ring}`}
                    title={a.derivation ?? a.prompt ?? a.role ?? ''}
                  >
                    {a.kind === 'audio' ? (
                      <div className="flex aspect-square w-full flex-col items-center justify-center gap-2 bg-surface-2/50 p-3">
                        <MusicNotesIcon size={26} weight="fill" className="text-teal-bright" />
                        <audio
                          controls
                          src={a.url}
                          onClick={(e) => e.stopPropagation()}
                          className="w-full"
                        />
                      </div>
                    ) : (
                      <AssetImage
                        src={a.url}
                        alt={a.prompt ?? a.role ?? ''}
                        className="aspect-square w-full object-cover"
                      />
                    )}

                    {/* Select checkbox (select mode) */}
                    {selecting && (
                      <span
                        className={`absolute left-1.5 top-1.5 grid size-5 place-items-center rounded-[6px] border transition ${
                          isSel ? 'border-indigo-bright bg-indigo-bright text-bg' : 'border-white/50 bg-black/40 text-transparent'
                        }`}
                      >
                        <CheckIcon size={12} weight="bold" />
                      </span>
                    )}

                    {!selecting && (
                      <span
                        className={`absolute left-1.5 top-1.5 rounded-[6px] px-1.5 py-0.5 text-[10px] font-medium ${
                          SOURCE_CHIP[a.source_kind] ?? 'bg-black/70 text-white/90'
                        }`}
                      >
                        {a.source_kind}
                      </span>
                    )}

                    {!selecting && a.exemplar && (
                      <span
                        className="absolute bottom-1.5 left-1.5 z-10 grid size-5 place-items-center rounded-[6px] bg-warning/85 text-bg"
                        title="Style exemplar — conditions new generations"
                      >
                        <StarIcon size={12} weight="fill" />
                      </span>
                    )}

                    {/* Status chip (colour + icon + word). Hides on hover/focus
                        when inspectable, so the inspect button can take its spot. */}
                    {(() => {
                      const c = STATUS_CHIP[a.status]
                      return (
                        <span
                          className={`pointer-events-none absolute right-1.5 top-1.5 inline-flex items-center gap-1 rounded-[6px] px-1.5 py-0.5 text-[10px] font-semibold ${c.cls} ${
                            !selecting ? 'transition group-hover:opacity-0 group-focus-within:opacity-0' : ''
                          }`}
                        >
                          <c.Icon size={11} weight="fill" className={c.pulse ? 'animate-pulse' : ''} />
                          {c.label}
                        </span>
                      )
                    })()}

                    {!selecting && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation()
                          setInspectId(a.id)
                        }}
                        aria-label={`Inspect ${api.displayName(a)}`}
                        className="absolute right-1.5 top-1.5 z-10 grid size-6 place-items-center rounded-[6px] bg-black/70 text-white/90 opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-bright/80"
                      >
                        <MagnifyingGlassIcon size={13} />
                      </button>
                    )}

                    {!selecting && a.status === 'candidate' && (
                      <div className="absolute inset-x-0 bottom-0 flex gap-1 bg-black/65 p-1.5 opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100">
                        {canApprove && (
                          <button
                            onClick={(e) => {
                              e.stopPropagation()
                              review(a.id, 'approved')
                            }}
                            aria-label="Approve"
                            className="flex flex-1 items-center justify-center rounded-[6px] bg-teal/85 py-1 text-bg transition hover:bg-teal"
                          >
                            <CheckIcon size={13} weight="bold" />
                          </button>
                        )}
                        <button
                          onClick={(e) => {
                            e.stopPropagation()
                            review(a.id, 'rejected')
                          }}
                          aria-label="Reject"
                          className="flex flex-1 items-center justify-center rounded-[6px] bg-white/10 py-1 text-danger transition hover:bg-white/20"
                        >
                          <XIcon size={13} weight="bold" />
                        </button>
                      </div>
                    )}

                    {/* QA gate: off-style flag (visual fit below the threshold) */}
                    {!selecting && a.style_fit != null && a.style_fit < api.STYLE_FIT_THRESHOLD && (
                      <span
                        title={`Off-style — ${Math.round(a.style_fit * 100)}% visual match to approved assets`}
                        className="absolute bottom-7 right-1.5 inline-flex items-center gap-1 rounded-[6px] bg-warning/85 px-1.5 py-0.5 text-[10px] font-semibold text-bg"
                      >
                        <WarningIcon size={11} weight="fill" />
                        {Math.round(a.style_fit * 100)}%
                      </span>
                    )}

                    <figcaption className="truncate px-2 py-1.5 text-[11px] text-text-dim">
                      {api.displayName(a)}
                    </figcaption>
                  </figure>
                )
              })}
            </div>
          )}

          {/* Load more — browse mode only (search returns a bounded ranked set). */}
          {searchHits == null && nextCursor && (
            <div className="mt-5 flex justify-center">
              <button
                onClick={loadMore}
                disabled={loadingMore}
                className="rounded-[8px] border border-white/10 px-4 py-2 text-sm text-text-dim transition hover:text-text disabled:opacity-50"
              >
                {loadingMore ? 'Loading more assets…' : 'Load more assets'}
              </button>
            </div>
          )}
        </PanelBody>
      </div>

      <AssetInspector
        assetId={inspectId}
        onClose={() => setInspectId(null)}
        onNavigate={setInspectId}
        onDeriveFrom={(id) => {
          setSelecting(false)
          setBaseId(id)
          setInspectId(null)
        }}
        onChanged={(updated) => {
          setAssets((a) => a.map((x) => (x.id === updated.id ? { ...x, ...updated } : x)))
          bumpFacets()
        }}
        onDeleted={(id) => {
          setAssets((a) => a.filter((x) => x.id !== id))
          if (baseId === id) setBaseId(null)
          bumpFacets()
        }}
      />

      {exportIds && (
        <ExportDialog
          projectId={projectId}
          assetIds={exportIds}
          title={`${exportIds.length} selected`}
          vertical={vertical}
          onClose={() => setExportIds(null)}
        />
      )}

      {confirm && (
        <ConfirmDialog
          title={confirm.title}
          body={confirm.body}
          confirmLabel={confirm.confirmLabel}
          tone={confirm.tone}
          onCancel={() => setConfirm(null)}
          onConfirm={() => {
            const run = confirm.run
            setConfirm(null)
            run()
          }}
        />
      )}
    </Panel>
  )
}
