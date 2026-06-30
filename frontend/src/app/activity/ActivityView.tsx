import { useEffect, useState } from 'react'
import {
  ClockCounterClockwiseIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { AssetInspector } from '../assets/AssetInspector'
import { Panel, PanelBody, PanelHeader, PanelIcon } from '../ui/Panel'

function ago(iso: string): string {
  const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000)
  if (s < 45) return 'just now'
  if (s < 3600) return `${Math.round(s / 60)}m`
  if (s < 86400) return `${Math.round(s / 3600)}h`
  if (s < 604800) return `${Math.round(s / 86400)}d`
  return new Date(iso).toLocaleDateString()
}

const ROW_BG = {
  asset: 'hover:bg-teal/8',
  comment: 'hover:bg-indigo/8',
  canon: 'hover:bg-warning/8',
} as const

const KIND_CHIP = {
  asset: 'bg-teal/15 text-teal-bright',
  comment: 'bg-indigo/15 text-indigo-bright',
  canon: 'bg-warning/15 text-warning',
} as const

const KIND_LABEL = {
  asset: 'Asset',
  comment: 'Comment',
  canon: 'Canon',
} as const

/**
 * Project activity — a read-only, time-sorted stream of recent asset creations,
 * comments, and canon versions. Asset/comment rows open the inspector.
 */
export function ActivityView({ projectId }: { projectId: string }) {
  const [events, setEvents] = useState<api.ActivityEvent[] | null>(null)
  const [inspectId, setInspectId] = useState<string | null>(null)

  useEffect(() => {
    let alive = true
    api.getActivity(projectId).then((e) => alive && setEvents(e)).catch(() => alive && setEvents([]))
    return () => {
      alive = false
    }
  }, [projectId])

  return (
    <Panel>
      <PanelHeader>
        <PanelIcon>
          <ClockCounterClockwiseIcon size={15} weight="fill" />
        </PanelIcon>
        <p className="text-sm font-medium text-text">Activity</p>
        {events && <span className="text-sm text-text-dim">· {events.length}</span>}
      </PanelHeader>

      <PanelBody>
        {events == null ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">Loading…</p>
        ) : events.length === 0 ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">
            No activity yet. Generate, derive, comment, or edit the canon to see it here.
          </p>
        ) : (
          <ol className="mx-auto max-w-2xl space-y-1">
            {events.map((e, i) => {
              const clickable = !!e.asset_id
              return (
                <li key={i}>
                  <button
                    onClick={() => e.asset_id && setInspectId(e.asset_id)}
                    disabled={!clickable}
                    className={`flex w-full items-start gap-3 rounded-[10px] px-2.5 py-2 text-left transition ${
                      clickable ? `${ROW_BG[e.kind]} cursor-pointer` : 'cursor-default'
                    }`}
                  >
                    <span
                      className={`mt-0.5 shrink-0 rounded-[6px] px-1.5 py-0.5 text-[10px] font-medium ${KIND_CHIP[e.kind]}`}
                    >
                      {KIND_LABEL[e.kind]}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-sm text-text-muted">{e.summary}</span>
                    <span className="mt-0.5 shrink-0 text-[10px] tabular-nums text-text-dim">{ago(e.at)}</span>
                  </button>
                </li>
              )
            })}
          </ol>
        )}
      </PanelBody>

      <AssetInspector
        assetId={inspectId}
        onClose={() => setInspectId(null)}
        onNavigate={setInspectId}
        onChanged={() => {}}
        onDeleted={() => setInspectId(null)}
      />
    </Panel>
  )
}
