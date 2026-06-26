import { useEffect, useMemo, useState } from 'react'
import {
  XIcon,
  SpinnerGapIcon,
  CheckCircleIcon,
  WarningIcon,
  DownloadSimpleIcon,
  PackageIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { ApiError } from '../../lib/api'

/**
 * Export dialog — runs the deterministic pre-export check on a set of assets,
 * shows a per-asset pass/fail report, then downloads the zip pack (manifest +
 * images). Blocking assets (rejected / undecodable) are excluded from the pack.
 */
export function ExportDialog({
  projectId,
  assetIds,
  title,
  onClose,
}: {
  projectId: string
  assetIds: string[]
  title: string
  onClose: () => void
}) {
  const [report, setReport] = useState<api.ExportReport | null>(null)
  const [loading, setLoading] = useState(true)
  const [downloading, setDownloading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let alive = true
    api
      .checkExport(projectId, assetIds)
      .then((r) => alive && setReport(r))
      .catch((e) => alive && setError(e instanceof ApiError ? e.message : 'Check failed.'))
      .finally(() => alive && setLoading(false))
    return () => {
      alive = false
    }
  }, [projectId, assetIds])

  // Group the report rows by their pack group (slugged role), first-seen order.
  const groupedEntries = useMemo<[string, api.AssetCheck[]][]>(() => {
    const m = new Map<string, api.AssetCheck[]>()
    for (const a of report?.assets ?? []) {
      const g = a.group || 'ungrouped'
      const arr = m.get(g) ?? []
      arr.push(a)
      m.set(g, arr)
    }
    return [...m.entries()]
  }, [report])

  async function download() {
    if (downloading) return
    setDownloading(true)
    setError(null)
    try {
      await api.downloadExport(projectId, assetIds)
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Download failed.')
    } finally {
      setDownloading(false)
    }
  }

  return (
    <>
      <div className="fixed inset-0 z-40 bg-black/50" onClick={onClose} aria-hidden />
      <div className="fixed left-1/2 top-1/2 z-50 flex max-h-[80vh] w-[520px] max-w-[94vw] -translate-x-1/2 -translate-y-1/2 flex-col rounded-[16px] border border-white/10 bg-surface-2 shadow-2xl">
        <header className="flex items-center gap-2 border-b border-white/8 px-4 py-3">
          <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
            <PackageIcon size={15} weight="fill" />
          </span>
          <p className="text-sm font-medium text-text">Export · {title}</p>
          <button
            onClick={onClose}
            aria-label="Close"
            className="ml-auto grid size-7 place-items-center rounded-[8px] text-text-dim transition hover:bg-white/5 hover:text-text"
          >
            <XIcon size={16} />
          </button>
        </header>

        {loading ? (
          <div className="grid place-items-center py-16 text-text-dim">
            <SpinnerGapIcon size={20} className="animate-spin" />
          </div>
        ) : !report ? (
          <p className="px-4 py-10 text-center text-sm text-rose-300">{error ?? 'No report.'}</p>
        ) : (
          <>
            <div className="flex items-center gap-3 border-b border-white/8 px-4 py-3 text-sm">
              <span className="inline-flex items-center gap-1.5 text-teal-bright">
                <CheckCircleIcon size={15} weight="fill" />
                {report.ok_count} ready
              </span>
              {report.assets.length - report.ok_count > 0 && (
                <span className="inline-flex items-center gap-1.5 text-amber-300">
                  <WarningIcon size={15} weight="fill" />
                  {report.assets.length - report.ok_count} skipped
                </span>
              )}
              <span className="ml-auto text-xs text-text-dim">{report.assets.length} selected</span>
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
              {groupedEntries.map(([group, items]) => (
                <div key={group} className="mb-3 last:mb-0">
                  <p className="mb-1 px-1 text-[10px] font-semibold uppercase tracking-wider text-text-dim">
                    {group} · {items.length}
                  </p>
                  <ul className="space-y-1.5">
                    {items.map((a) => (
                      <li
                        key={a.id}
                        className={`flex items-start gap-2 rounded-[10px] px-2.5 py-2 text-xs ${
                          a.ok ? 'bg-white/[0.03]' : 'bg-amber-400/8'
                        }`}
                      >
                        {a.ok ? (
                          <CheckCircleIcon size={15} weight="fill" className="mt-0.5 shrink-0 text-teal" />
                        ) : (
                          <WarningIcon size={15} weight="fill" className="mt-0.5 shrink-0 text-amber-300" />
                        )}
                        <div className="min-w-0 flex-1">
                          <p className="flex items-center gap-2">
                            <span className="truncate font-medium text-text">{a.filename}</span>
                            {a.width && a.height ? (
                              <span className="shrink-0 text-[10px] text-text-dim">
                                {a.width}×{a.height}
                                {a.has_alpha ? ' · alpha' : ''}
                              </span>
                            ) : null}
                          </p>
                          {a.issues.length > 0 && (
                            <p className="mt-0.5 text-[11px] text-text-dim">{a.issues.join(' · ')}</p>
                          )}
                        </div>
                      </li>
                    ))}
                  </ul>
                </div>
              ))}
            </div>

            <footer className="flex items-center gap-3 border-t border-white/8 px-4 py-3">
              {error && <span className="text-xs text-rose-300">{error}</span>}
              <button
                onClick={download}
                disabled={downloading || report.ok_count === 0}
                className="ml-auto inline-flex items-center gap-1.5 rounded-[8px] bg-teal px-4 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
                title={report.ok_count === 0 ? 'Nothing exportable' : 'Download the zip pack'}
              >
                {downloading ? (
                  <SpinnerGapIcon size={14} className="animate-spin" />
                ) : (
                  <DownloadSimpleIcon size={14} weight="bold" />
                )}
                Download pack ({report.ok_count})
              </button>
            </footer>
          </>
        )}
      </div>
    </>
  )
}
