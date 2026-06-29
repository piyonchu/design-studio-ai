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
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'
import { AssetInspector } from './AssetInspector'
import { ExportDialog } from '../export/ExportDialog'
import { JobsBanner } from './JobsBanner'
import { verticalConfig } from '../verticals'

const STATUSES: api.AssetStatus[] = ['candidate', 'approved', 'needs_review', 'rejected']

// Status as visual language on the board: candidate = dashed amber, approved =
// solid teal, needs_review = rose (with a pulsing flag), rejected = dimmed.
const STATUS_RING: Record<api.AssetStatus, string> = {
  candidate: 'ring-1 ring-amber-400/45',
  approved: 'ring-2 ring-teal/70',
  needs_review: 'ring-2 ring-rose-400/55',
  rejected: 'ring-1 ring-white/10 opacity-50',
}
const STATUS_DOT: Record<api.AssetStatus, string> = {
  candidate: 'bg-amber-400',
  approved: 'bg-teal',
  needs_review: 'bg-rose-400',
  rejected: 'bg-white/30',
}

/**
 * The smart asset board — generate / upload / derive plus a filter rail
 * (role · status · source · collection), free-text search, status visual
 * language, and multi-select batch actions (approve / add-to-collection).
 * Click a tile to pick a derivation base; toggle Select for batch mode.
 */
export function AssetLibrary({ projectId, vertical }: { projectId: string; vertical?: string }) {
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

  // Multi-select
  const [selecting, setSelecting] = useState(false)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [batchCol, setBatchCol] = useState('')
  const [exportIds, setExportIds] = useState<string[] | null>(null)

  const [recipes, setRecipes] = useState<api.Recipe[]>([])
  // Keyset pagination + whole-project facet counts (so the rail stays accurate
  // even though only a page of assets is loaded).
  const [nextCursor, setNextCursor] = useState<string | null>(null)
  const [loadingMore, setLoadingMore] = useState(false)
  const [facets, setFacets] = useState<api.AssetFacets | null>(null)
  const [facetsBump, setFacetsBump] = useState(0)
  const bumpFacets = () => setFacetsBump((n) => n + 1)
  const PAGE = 50

  useEffect(() => {
    api.listCollections(projectId).then(setCollections).catch(() => {})
    api.listRecipes(projectId).then(setRecipes).catch(() => {})
  }, [projectId])

  // Filter-rail counts over the whole project; refetched after mutations.
  useEffect(() => {
    api.getAssetFacets(projectId).then(setFacets).catch(() => {})
  }, [projectId, facetsBump])

  // First page — reloads whenever a server-side filter changes. Server does the
  // filtering + ordering; we just render the page.
  useEffect(() => {
    let alive = true
    api
      .listAssets(projectId, {
        limit: PAGE,
        status: [...statuses],
        role: [...roles],
        source: [...sources],
        collection: collectionId,
      })
      .then((page) => {
        if (!alive) return
        setAssets(page.items)
        setNextCursor(page.next_cursor)
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [projectId, statuses, roles, sources, collectionId])

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
      })
      setAssets((a) => [...a, ...page.items])
      setNextCursor(page.next_cursor)
    } catch {
      /* ignore */
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
      setError(err instanceof ApiError ? err.message : 'Save recipe failed.')
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
      return true
    })
  }, [assets, searchHits, roles, statuses, sources, collectionId, collMembers])

  const activeFilters = roles.size + statuses.size + sources.size + (collectionId ? 1 : 0) + (query.trim() ? 1 : 0)

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
  }

  function genError(err: unknown) {
    // Surface the server's message — already user-readable and specific:
    // not-configured (503), the cost guardrail (503 credit floor / 429 daily
    // cap), or content policy (400). A generic fallback otherwise.
    setError(err instanceof ApiError ? err.message : 'Request failed.')
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
          })
          .then((page) => {
            setAssets(page.items)
            setNextCursor(page.next_cursor)
          })
          .catch(() => {})
        bumpFacets()
        return
      }
      if (job.status === 'failed') {
        setError(job.error || 'Generation failed.')
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
    setBusy(true)
    setError(null)
    try {
      const created = await api.uploadAsset(projectId, file, 'base')
      setAssets((a) => [created, ...a])
      bumpFacets()
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Upload failed.')
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
    try {
      for (const p of PRESETS) {
        try {
          const created = await api.deriveAssets(projectId, baseId, p.text, 1)
          setAssets((a) => [...created, ...a])
        } catch (err) {
          genError(err)
        }
      }
      bumpFacets()
    } finally {
      setBusy(false)
    }
  }

  async function review(id: string, status: api.AssetStatus) {
    try {
      const updated = await api.setAssetStatus(id, status)
      setAssets((a) => a.map((x) => (x.id === id ? updated : x)))
      bumpFacets()
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Update failed.')
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
      setError(err instanceof ApiError ? err.message : 'Batch update failed.')
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
      setError(err instanceof ApiError ? err.message : 'Add failed.')
    } finally {
      setBusy(false)
    }
  }

  // ── Rail building blocks ────────────────────────────────────────────────────
  function FilterChip({ active, count, onClick, children }: {
    active: boolean
    count?: number
    onClick: () => void
    children: ReactNode
  }) {
    return (
      <button
        onClick={onClick}
        className={`flex w-full items-center gap-2 rounded-[8px] px-2.5 py-1.5 text-left text-xs capitalize transition ${
          active ? 'bg-teal/15 text-teal-bright' : 'text-text-dim hover:bg-white/5 hover:text-text'
        }`}
      >
        <span className="flex-1 truncate">{children}</span>
        {count != null && <span className="text-[10px] tabular-nums text-text-dim">{count}</span>}
      </button>
    )
  }

  function Section({ title, children }: { title: string; children: ReactNode }) {
    return (
      <div className="mb-4">
        <p className="mb-1 px-2.5 text-[10px] font-semibold uppercase tracking-wider text-text-dim">{title}</p>
        {children}
      </div>
    )
  }

  return (
    <div className="glass flex min-h-0 flex-1 overflow-hidden rounded-[16px]">
      {/* Filter rail */}
      <aside className="flex w-56 shrink-0 flex-col border-r border-white/8">
        <div className="border-b border-white/8 p-3">
          <div className="flex items-center gap-2 rounded-[10px] bg-surface-2/60 px-2.5 py-2">
            <MagnifyingGlassIcon size={14} className="text-text-dim" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search assets…"
              className="w-full bg-transparent text-xs text-text outline-none placeholder:text-text-dim"
            />
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          <Section title="Status">
            {STATUSES.map((s) => (
              <FilterChip
                key={s}
                active={statuses.has(s)}
                count={statusCounts.get(s) ?? 0}
                onClick={() => setStatuses((set) => toggle(set, s))}
              >
                <span className="inline-flex items-center gap-1.5">
                  <span className={`size-2 rounded-full ${STATUS_DOT[s]}`} />
                  {s.replace('_', ' ')}
                </span>
              </FilterChip>
            ))}
          </Section>

          {roleOptions.length > 0 && (
            <Section title="Role">
              {roleOptions.map(([r, n]) => (
                <FilterChip key={r} active={roles.has(r)} count={n} onClick={() => setRoles((set) => toggle(set, r))}>
                  {r}
                </FilterChip>
              ))}
            </Section>
          )}

          <Section title="Source">
            {sourceOptions.map(([s, n]) => (
              <FilterChip key={s} active={sources.has(s)} count={n} onClick={() => setSources((set) => toggle(set, s))}>
                {s}
              </FilterChip>
            ))}
          </Section>

          {collections.length > 0 && (
            <Section title="Collection">
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
            </Section>
          )}
        </div>

        {activeFilters > 0 && (
          <button
            onClick={clearFilters}
            className="border-t border-white/8 px-3 py-2.5 text-left text-xs text-text-dim transition hover:text-text"
          >
            Clear {activeFilters} filter{activeFilters > 1 ? 's' : ''}
          </button>
        )}
      </aside>

      {/* Main column */}
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="flex items-center gap-2 border-b border-white/8 px-5 py-4">
          <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
            <ImageIcon size={15} weight="fill" />
          </span>
          <p className="text-sm font-medium text-text">Asset Board</p>
          <span className="text-sm text-text-dim">
            · {displayed.length}
            {searchHits == null && nextCursor && '+'}
          </span>
          <button
            onClick={toggleSelectMode}
            className={`ml-auto inline-flex items-center gap-1.5 rounded-[8px] border px-3 py-1.5 text-sm transition ${
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
        </div>

        {/* Batch toolbar (select mode) — else generate / derive bar */}
        {selecting ? (
          <div className="flex flex-wrap items-center gap-2 border-b border-white/8 px-5 py-3">
            <span className="text-sm text-text">{selected.size} selected</span>
            <button
              onClick={() => batchStatus('approved')}
              disabled={!selected.size || busy}
              className="inline-flex items-center gap-1.5 rounded-[8px] bg-teal px-3 py-1.5 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-40"
            >
              <CheckIcon size={14} weight="bold" />
              Approve
            </button>
            <button
              onClick={() => batchStatus('rejected')}
              disabled={!selected.size || busy}
              className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/10 px-3 py-1.5 text-sm text-rose-200 transition hover:bg-white/5 disabled:opacity-40"
            >
              <XIcon size={14} weight="bold" />
              Reject
            </button>
            {collections.length > 0 && (
              <div className="flex items-center gap-1.5">
                <select
                  value={batchCol}
                  onChange={(e) => setBatchCol(e.target.value)}
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
          </div>
        ) : baseId ? (
          <div className="border-b border-white/8 p-4">
            <div className="mx-auto max-w-2xl">
              <div className="mb-2 flex items-center gap-2 text-xs text-text-dim">
                <span>Deriving from selected base — pick a preset or write an instruction</span>
                <button
                  onClick={deriveAll}
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
                        className="text-text-dim opacity-0 transition hover:text-rose-300 group-hover:opacity-100"
                      >
                        <XIcon size={11} />
                      </button>
                    </span>
                  ))}
                </div>
              )}
              <div className="flex items-center gap-2 rounded-[12px] bg-surface-2/60 p-2">
                <input
                  value={instruction}
                  onChange={(e) => setInstruction(e.target.value)}
                  placeholder="Derivation instruction…"
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
        ) : (
          <form onSubmit={generate} className="border-b border-white/8 p-4">
            <div className="mx-auto flex max-w-2xl items-center gap-2 rounded-[12px] bg-surface-2/60 p-2">
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
        )}

        {error && <p className="px-5 pt-3 text-xs text-rose-300">{error}</p>}

        {!selecting && !baseId && dupHits.length > 0 && (
          <div className="mx-5 mt-3 flex items-center gap-3 rounded-[10px] border border-amber-400/25 bg-amber-400/8 px-3 py-2">
            <WarningIcon size={16} weight="fill" className="shrink-0 text-amber-300" />
            <p className="shrink-0 text-xs text-amber-100">
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
                  <img src={s.url} alt="" className="size-full object-cover" />
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
        )}

        <div className="min-h-0 flex-1 overflow-y-auto p-5">
          <JobsBanner projectId={projectId} />
          {displayed.length === 0 ? (
            <p className="px-1 py-16 text-center text-sm text-text-dim">
              {activeFilters > 0
                ? 'No assets match these filters.'
                : 'No assets yet. Generate one above, or upload a base.'}
            </p>
          ) : (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
              {displayed.map((a) => {
                const isSel = selected.has(a.id)
                const isBase = !selecting && a.id === baseId
                // Selection uses sky (multi-select and derive base are separate
                // modes, never shown together) — distinct from the status rings
                // (approved=teal, candidate=amber, needs_review=rose).
                const ring = isSel || isBase
                  ? 'ring-2 ring-sky-400'
                  : STATUS_RING[a.status]
                return (
                  <figure
                    key={a.id}
                    onClick={() => onTileClick(a)}
                    className={`group relative cursor-pointer overflow-hidden rounded-[12px] transition ${ring}`}
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
                      <img src={a.url} alt={a.prompt ?? a.role ?? ''} className="aspect-square w-full object-cover" />
                    )}

                    {/* Select checkbox (select mode) */}
                    {selecting && (
                      <span
                        className={`absolute left-1.5 top-1.5 grid size-5 place-items-center rounded-[6px] border transition ${
                          isSel ? 'border-sky-400 bg-sky-400 text-bg' : 'border-white/50 bg-black/40 text-transparent'
                        }`}
                      >
                        <CheckIcon size={12} weight="bold" />
                      </span>
                    )}

                    {!selecting && (
                      <span className="absolute left-1.5 top-1.5 rounded-[6px] bg-black/55 px-1.5 py-0.5 text-[10px] font-medium text-white/90 backdrop-blur">
                        {a.source_kind}
                      </span>
                    )}

                    {!selecting && a.exemplar && (
                      <span
                        className="absolute bottom-1.5 left-1.5 z-10 grid size-5 place-items-center rounded-[6px] bg-amber-400/85 text-bg"
                        title="Style exemplar — conditions new generations"
                      >
                        <StarIcon size={12} weight="fill" />
                      </span>
                    )}

                    {/* needs_review pulsing flag */}
                    {a.status === 'needs_review' && (
                      <span className="absolute right-1.5 top-1.5 grid size-6 place-items-center rounded-[6px] bg-rose-500/80 text-white">
                        <FlagIcon size={13} weight="fill" className="animate-pulse" />
                      </span>
                    )}
                    {a.status !== 'needs_review' && a.status !== 'candidate' && (
                      <span className="absolute right-1.5 top-1.5 rounded-[6px] bg-teal/80 px-1.5 py-0.5 text-[10px] font-medium text-bg transition group-hover:opacity-0">
                        {a.status}
                      </span>
                    )}

                    {!selecting && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation()
                          setInspectId(a.id)
                        }}
                        aria-label="Inspect"
                        className="absolute right-1.5 top-1.5 z-10 grid size-6 place-items-center rounded-[6px] bg-black/55 text-white/90 opacity-0 backdrop-blur transition group-hover:opacity-100"
                      >
                        <MagnifyingGlassIcon size={13} />
                      </button>
                    )}

                    {!selecting && a.status === 'candidate' && (
                      <div className="absolute inset-x-0 bottom-0 flex gap-1 bg-black/45 p-1.5 opacity-0 backdrop-blur transition group-hover:opacity-100">
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
                        <button
                          onClick={(e) => {
                            e.stopPropagation()
                            review(a.id, 'rejected')
                          }}
                          aria-label="Reject"
                          className="flex flex-1 items-center justify-center rounded-[6px] bg-white/10 py-1 text-rose-200 transition hover:bg-white/20"
                        >
                          <XIcon size={13} weight="bold" />
                        </button>
                      </div>
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
                {loadingMore ? 'Loading…' : 'Load more'}
              </button>
            </div>
          )}
        </div>
      </div>

      <AssetInspector
        assetId={inspectId}
        onClose={() => setInspectId(null)}
        onNavigate={setInspectId}
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
    </div>
  )
}
