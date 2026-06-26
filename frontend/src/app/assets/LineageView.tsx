import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  TreeStructureIcon,
  WarningIcon,
  ArrowsClockwiseIcon,
  SpinnerGapIcon,
  MagnifyingGlassIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'
import { AssetInspector } from './AssetInspector'

const STATUS_DOT: Record<api.AssetStatus, string> = {
  candidate: 'bg-amber-400',
  approved: 'bg-teal',
  needs_review: 'bg-rose-400',
  rejected: 'bg-white/30',
}

/**
 * Lineage view — the moat made visible. Lays out roots → derivatives from the
 * project graph, and surfaces canon drift: assets generated under an older
 * canon are flagged stale, with "keep" (rebind to current canon) or
 * "regenerate" (re-run generate/derive under the new canon as a fresh
 * candidate). Click any node to inspect it.
 */
export function LineageView({ projectId }: { projectId: string }) {
  const [graph, setGraph] = useState<api.LineageGraph | null>(null)
  const [canonId, setCanonId] = useState<string | null>(null)
  const [canonVersion, setCanonVersion] = useState<number | null>(null)
  const [busy, setBusy] = useState(false)
  const [regenId, setRegenId] = useState<string | null>(null)
  const [inspectId, setInspectId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(() => {
    return Promise.all([api.getLineage(projectId), api.getCanon(projectId)])
      .then(([g, c]) => {
        setGraph(g)
        setCanonId(c?.id ?? null)
        setCanonVersion((c as { version?: number } | null)?.version ?? null)
      })
      .catch((e) => setError(e instanceof ApiError ? e.message : 'Failed to load lineage.'))
  }, [projectId])

  useEffect(() => {
    load()
  }, [load])

  const assetById = useMemo(() => {
    const m = new Map<string, api.Asset>()
    for (const a of graph?.assets ?? []) m.set(a.id, a)
    return m
  }, [graph])

  // base id (to_asset) → its derivatives; and the set of assets that are derivatives.
  const { childrenOf, baseOf, derivedSet } = useMemo(() => {
    const childrenOf = new Map<string, api.Asset[]>()
    const baseOf = new Map<string, string>()
    const derivedSet = new Set<string>()
    for (const l of graph?.links ?? []) {
      const child = assetById.get(l.from_asset)
      if (!child) continue
      derivedSet.add(l.from_asset)
      baseOf.set(l.from_asset, l.to_asset)
      const arr = childrenOf.get(l.to_asset) ?? []
      arr.push(child)
      childrenOf.set(l.to_asset, arr)
    }
    return { childrenOf, baseOf, derivedSet }
  }, [graph, assetById])

  // Roots: assets not derived from anything (uploads, seeds, orphans).
  const roots = useMemo(
    () => (graph?.assets ?? []).filter((a) => !derivedSet.has(a.id)),
    [graph, derivedSet],
  )

  const isStale = useCallback(
    (a: api.Asset) => canonId != null && a.canon_version_id != null && a.canon_version_id !== canonId,
    [canonId],
  )

  const staleAssets = useMemo(() => (graph?.assets ?? []).filter(isStale), [graph, isStale])

  async function keepAll() {
    if (busy || !staleAssets.length) return
    setBusy(true)
    setError(null)
    try {
      await api.reconcileAssets(projectId, staleAssets.map((a) => a.id))
      await load()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Reconcile failed.')
    } finally {
      setBusy(false)
    }
  }

  async function keepOne(id: string) {
    setError(null)
    try {
      await api.reconcileAssets(projectId, [id])
      await load()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Reconcile failed.')
    }
  }

  // Regenerate under the current canon: re-derive (if it has a base) or
  // re-generate from the prompt. Produces a fresh candidate; the stale one stays.
  async function regenerate(a: api.Asset) {
    if (regenId) return
    setRegenId(a.id)
    setError(null)
    try {
      const base = baseOf.get(a.id)
      if (a.source_kind === 'derived' && base && a.derivation) {
        await api.deriveAssets(projectId, base, a.derivation, 1)
      } else if (a.prompt) {
        await api.generateAssets(projectId, a.prompt, 1)
      } else {
        setError('Nothing to regenerate from (no base or prompt).')
        return
      }
      await load()
    } catch (e) {
      setError(
        e instanceof ApiError && e.status === 503
          ? 'Generation unavailable. (Set OPENROUTER_API_KEY, or ASSET_MOCK=true.)'
          : e instanceof ApiError
            ? e.message
            : 'Regenerate failed.',
      )
    } finally {
      setRegenId(null)
    }
  }

  function Node({ asset, depth, seen }: { asset: api.Asset; depth: number; seen: Set<string> }) {
    if (seen.has(asset.id)) return null
    const next = new Set(seen).add(asset.id)
    const kids = childrenOf.get(asset.id) ?? []
    const stale = isStale(asset)
    return (
      <div className="flex flex-col">
        <div className="flex items-center gap-3">
          <figure
            onClick={() => setInspectId(asset.id)}
            className={`group relative w-44 shrink-0 cursor-pointer overflow-hidden rounded-[12px] ring-1 transition ${
              stale ? 'ring-amber-400/50' : 'ring-white/10 hover:ring-white/25'
            }`}
            title={asset.derivation ?? asset.prompt ?? asset.role ?? ''}
          >
            <div className="flex items-center gap-2.5 p-2">
              <img src={asset.url} alt="" className="size-12 shrink-0 rounded-[8px] object-cover ring-1 ring-white/10" />
              <div className="min-w-0 flex-1">
                <p className="truncate text-xs text-text">{asset.role ?? asset.prompt ?? asset.derivation ?? 'untitled'}</p>
                <p className="mt-1 inline-flex items-center gap-1.5 text-[10px] text-text-dim">
                  <span className={`size-1.5 rounded-full ${STATUS_DOT[asset.status]}`} />
                  {asset.status.replace('_', ' ')} · {asset.source_kind}
                </p>
              </div>
              <MagnifyingGlassIcon size={13} className="text-text-dim opacity-0 transition group-hover:opacity-100" />
            </div>
            {stale && (
              <span className="absolute right-1.5 top-1.5 rounded-[5px] bg-amber-400/20 px-1.5 py-0.5 text-[9px] font-medium text-amber-200">
                stale
              </span>
            )}
          </figure>

          {stale && (
            <div className="flex shrink-0 items-center gap-1">
              <button
                onClick={() => regenerate(asset)}
                disabled={regenId != null}
                title="Regenerate under the current canon"
                className="inline-flex items-center gap-1 rounded-[8px] border border-white/10 px-2 py-1 text-[11px] text-text-dim transition hover:text-text disabled:opacity-50"
              >
                {regenId === asset.id ? (
                  <SpinnerGapIcon size={12} className="animate-spin" />
                ) : (
                  <ArrowsClockwiseIcon size={12} />
                )}
                Regenerate
              </button>
              <button
                onClick={() => keepOne(asset.id)}
                title="Keep as-is under the current canon"
                className="rounded-[8px] border border-white/10 px-2 py-1 text-[11px] text-text-dim transition hover:text-text"
              >
                Keep
              </button>
            </div>
          )}
        </div>

        {kids.length > 0 && (
          <div className="ml-6 mt-2 flex flex-col gap-2 border-l border-white/10 pl-4">
            {kids.map((k) => (
              <Node key={k.id} asset={k} depth={depth + 1} seen={next} />
            ))}
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="glass flex min-h-0 flex-1 flex-col rounded-[16px]">
      <div className="flex items-center gap-2 border-b border-white/8 px-5 py-4">
        <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
          <TreeStructureIcon size={15} weight="fill" />
        </span>
        <p className="text-sm font-medium text-text">Lineage</p>
        {graph && <span className="text-sm text-text-dim">· {graph.assets.length} assets</span>}
        {canonVersion != null && <span className="ml-auto text-xs text-text-dim">canon v{canonVersion}</span>}
      </div>

      {staleAssets.length > 0 && (
        <div className="mx-5 mt-4 flex flex-wrap items-center gap-3 rounded-[12px] border border-amber-400/25 bg-amber-400/8 px-4 py-3">
          <WarningIcon size={18} className="text-amber-300" weight="fill" />
          <p className="text-sm text-amber-100">
            {staleAssets.length} asset{staleAssets.length > 1 ? 's' : ''} predate the current canon
            {canonVersion != null ? ` (v${canonVersion})` : ''}.
          </p>
          <span className="text-xs text-text-dim">Regenerate to restyle, or keep to accept as-is.</span>
          <button
            onClick={keepAll}
            disabled={busy}
            className="ml-auto inline-flex items-center gap-1.5 rounded-[8px] bg-teal px-3 py-1.5 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
          >
            {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : null}
            Keep all
          </button>
        </div>
      )}

      {error && <p className="px-5 pt-3 text-xs text-rose-300">{error}</p>}

      <div className="min-h-0 flex-1 overflow-auto p-5">
        {!graph ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">Loading…</p>
        ) : graph.assets.length === 0 ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">
            No assets yet. Generate or derive assets to see their lineage here.
          </p>
        ) : (
          <div className="flex flex-col gap-4">
            {roots.map((r) => (
              <Node key={r.id} asset={r} depth={0} seen={new Set()} />
            ))}
          </div>
        )}
      </div>

      <AssetInspector
        assetId={inspectId}
        onClose={() => setInspectId(null)}
        onNavigate={setInspectId}
        onChanged={() => load()}
        onDeleted={() => {
          setInspectId(null)
          load()
        }}
      />
    </div>
  )
}
