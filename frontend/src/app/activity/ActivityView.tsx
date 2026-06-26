import { useEffect, useState } from 'react'
import {
  ImageIcon,
  ChatCircleIcon,
  PaletteIcon,
  ClockCounterClockwiseIcon,
} from '@phosphor-icons/react'
import * as api from '../../lib/api'
import { AssetInspector } from '../assets/AssetInspector'

function ago(iso: string): string {
  const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000)
  if (s < 45) return 'just now'
  if (s < 3600) return `${Math.round(s / 60)}m`
  if (s < 86400) return `${Math.round(s / 3600)}h`
  if (s < 604800) return `${Math.round(s / 86400)}d`
  return new Date(iso).toLocaleDateString()
}

const ICON = {
  asset: ImageIcon,
  comment: ChatCircleIcon,
  canon: PaletteIcon,
} as const

const TINT = {
  asset: 'text-teal-bright',
  comment: 'text-indigo-bright',
  canon: 'text-amber-300',
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
    <div className="glass flex min-h-0 flex-1 flex-col rounded-[16px]">
      <div className="flex items-center gap-2 border-b border-white/8 px-5 py-4">
        <span className="grid size-7 place-items-center rounded-[8px] bg-accent/15 text-teal-bright">
          <ClockCounterClockwiseIcon size={15} weight="fill" />
        </span>
        <p className="text-sm font-medium text-text">Activity</p>
        {events && <span className="text-sm text-text-dim">· {events.length}</span>}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-5">
        {events == null ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">Loading…</p>
        ) : events.length === 0 ? (
          <p className="px-1 py-16 text-center text-sm text-text-dim">
            No activity yet. Generate, derive, comment, or edit the canon to see it here.
          </p>
        ) : (
          <ol className="mx-auto max-w-2xl space-y-1">
            {events.map((e, i) => {
              const Icon = ICON[e.kind]
              const clickable = !!e.asset_id
              return (
                <li key={i}>
                  <button
                    onClick={() => e.asset_id && setInspectId(e.asset_id)}
                    disabled={!clickable}
                    className={`flex w-full items-start gap-3 rounded-[10px] px-2.5 py-2 text-left transition ${
                      clickable ? 'hover:bg-white/5' : 'cursor-default'
                    }`}
                  >
                    <span className={`mt-0.5 shrink-0 ${TINT[e.kind]}`}>
                      <Icon size={16} weight="fill" />
                    </span>
                    <span className="min-w-0 flex-1 truncate text-sm text-text-muted">{e.summary}</span>
                    <span className="mt-0.5 shrink-0 text-[10px] tabular-nums text-text-dim">{ago(e.at)}</span>
                  </button>
                </li>
              )
            })}
          </ol>
        )}
      </div>

      <AssetInspector
        assetId={inspectId}
        onClose={() => setInspectId(null)}
        onNavigate={setInspectId}
        onChanged={() => {}}
        onDeleted={() => setInspectId(null)}
      />
    </div>
  )
}
