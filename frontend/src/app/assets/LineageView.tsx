import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  TreeStructureIcon,
  WarningIcon,
  ArrowsClockwiseIcon,
  SpinnerGapIcon,
  MagnifyingGlassIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError, formatApiError } from '../../lib/api'
import { AssetInspector } from './AssetInspector'
import { Panel, PanelBody, PanelHeader, PanelIcon, PanelInset } from '../ui/Panel'
import { ErrorBanner } from '../ui/ErrorBanner'

const STATUS_DOT: Record<api.AssetStatus, string> = {
  candidate: 'bg-warning',
  approved: 'bg-teal',
  needs_review: 'bg-danger',
  rejected: 'bg-white/30',
}

type LineageCtx = {
  childrenOf: Map<string, api.Asset[]>
  isStale: (a: api.Asset) => boolean
  regenId: string | null
  onInspect: (id: string) => void
  onRegenerate: (a: api.Asset) => void
  onKeep: (id: string) => void
}

// One lineage node + its subtree. Module-scoped (stable component identity) so
// the tree reconciles in place instead of remounting on each state change.
function LineageNode({ asset, seen, ctx }: { asset: api.Asset; seen: Set<string>; ctx: LineageCtx }) {
  if (seen.has(asset.id)) return null
  const next = new Set(seen).add(asset.id)
  const kids = ctx.childrenOf.get(asset.id) ?? []
  const stale = ctx.isStale(asset)
  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-3">
        <figure
          onClick={() => ctx.onInspect(asset.id)}
          className={`group relative w-44 shrink-0 cursor-pointer overflow-hidden rounded-[12px] ring-1 transition ${
            stale ? 'ring-warning/50' : 'ring-white/10 hover:ring-white/25'
          }`}
          title={asset.derivation ?? asset.prompt ?? asset.role ?? ''}
        >
          <div className="flex items-center gap-2.5 p-2">
            <img src={asset.url} alt="" loading="lazy" decoding="async" className="size-12 shrink-0 rounded-[8px] object-cover ring-1 ring-white/10" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-xs text-text">{api.displayName(asset)}</p>
              <p className="mt-1 inline-flex items-center gap-1.5 text-[10px] text-text-dim">
                <span className={`size-1.5 rounded-full ${STATUS_DOT[asset.status]}`} />
                {asset.status.replace('_', ' ')} · {asset.source_kind}
              </p>
            </div>
            <MagnifyingGlassIcon size={13} className="text-text-dim opacity-0 transition group-hover:opacity-100" />
          </div>
          {stale && (
            <span className="absolute right-1.5 top-1.5 rounded-[5px] bg-warning/20 px-1.5 py-0.5 text-[9px] font-medium text-warning">
              stale
            </span>
          )}
        </figure>

        {stale && (
          <div className="flex shrink-0 items-center gap-1">
            <button
              onClick={() => ctx.onRegenerate(asset)}
              disabled={ctx.regenId != null}
              title="Regenerate under the current canon"
              className="inline-flex items-center gap-1 rounded-[8px] border border-white/10 px-2 py-1 text-[11px] text-text-dim transition hover:text-text disabled:opacity-50"
            >
              {ctx.regenId === asset.id ? (
                <SpinnerGapIcon size={12} className="animate-spin" />
              ) : (
                <ArrowsClockwiseIcon size={12} />
              )}
              Regenerate
            </button>
            <button
              onClick={() => ctx.onKeep(asset.id)}
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
            <LineageNode key={k.id} asset={k} seen={next} ctx={ctx} />
          ))}
        </div>
      )}
    </div>
  )
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
      .catch((e) => setError(formatApiError(e, "Couldn't load lineage. Try again.")))
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
      setError(formatApiError(e, "Couldn't rebind assets to the current canon. Try again."))
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
      setError(formatApiError(e, "Couldn't rebind assets to the current canon. Try again."))
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
        setError('This asset has no base or prompt to regenerate from.')
        return
      }
      await load()
    } catch (e) {
      if (e instanceof ApiError && e.status === 503) {
        setError('Generation is unavailable. Ask your workspace admin to configure the API key.')
      } else {
        setError(formatApiError(e, "Couldn't regenerate this asset. Try again."))
      }
    } finally {
      setRegenId(null)
    }
  }

  // Stable ctx for the module-level LineageNode — keeps the recursive tree from
  // remounting on every regen/inspect state change (it re-renders in place).
  const nodeCtx = useMemo<LineageCtx>(
    () => ({ childrenOf, isStale, regenId, onInspect: setInspectId, onRegenerate: regenerate, onKeep: keepOne }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [childrenOf, isStale, regenId],
  )

  return (
    <Panel>
      <PanelHeader>
        <PanelIcon>
          <TreeStructureIcon size={15} weight="fill" />
        </PanelIcon>
        <p className="text-sm font-medium text-text">Lineage</p>
        {graph && <span className="text-sm text-text-dim">· {graph.assets.length} assets</span>}
        {canonVersion != null && (
          <span className="ml-auto rounded-[6px] bg-teal/12 px-2 py-0.5 text-xs font-medium text-teal-bright ring-1 ring-teal/25">
            canon v{canonVersion}
          </span>
        )}
      </PanelHeader>

      {staleAssets.length > 0 && (
        <PanelInset>
        <div className="flex flex-wrap items-center gap-3 rounded-[12px] border border-warning/25 bg-warning/8 px-4 py-3">
          <WarningIcon size={18} className="text-warning" weight="fill" />
          <p className="text-sm text-warning">
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
        </PanelInset>
      )}

      {error && (
        <PanelInset>
          <ErrorBanner message={error} onDismiss={() => setError(null)} />
        </PanelInset>
      )}

      <PanelBody scroll={false} className="overflow-auto">
        {!graph ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">Loading lineage…</p>
        ) : graph.assets.length === 0 ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">
            No lineage yet. Generate or derive assets on the board to map their relationships here.
          </p>
        ) : (
          <div className="flex flex-col gap-4">
            {roots.map((r) => (
              <LineageNode key={r.id} asset={r} seen={new Set()} ctx={nodeCtx} />
            ))}
          </div>
        )}
      </PanelBody>

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
    </Panel>
  )
}
