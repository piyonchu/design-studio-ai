import { useEffect, useState } from 'react'
import { LightningIcon } from '@phosphor-icons/react'
import * as api from '../lib/api'

/**
 * A small dev-visibility chip showing the shared OpenRouter key's remaining
 * credit (the budget real generation draws down). Silent on error — it's an
 * ambient indicator, not a blocking control. The backend caches the value, so
 * mounting this often is cheap. Later this seam can show per-workspace quota.
 */
export function CreditChip() {
  const [usage, setUsage] = useState<api.Usage | null>(null)

  useEffect(() => {
    let alive = true
    api.getUsage().then((u) => alive && setUsage(u)).catch(() => {})
    return () => {
      alive = false
    }
  }, [])

  if (!usage) return null

  // Color by how much of the budget is left (when a limit is known).
  const frac = usage.limit ? usage.remaining / usage.limit : 1
  const tone = frac < 0.1 ? 'text-rose-300' : frac < 0.25 ? 'text-amber-300' : 'text-text-dim'
  const live = usage.source !== 'mock'

  return (
    <span
      className="inline-flex items-center gap-1.5 rounded-[8px] border border-white/8 bg-surface-2/50 px-2.5 py-1.5 text-xs"
      title={
        live
          ? `OpenRouter key: $${usage.remaining.toFixed(2)} of $${(usage.limit ?? 0).toFixed(0)} left · $${usage.usage.toFixed(2)} used${usage.source === 'stale' ? ' (cached)' : ''}`
          : 'Mock budget (no OPENROUTER_API_KEY)'
      }
    >
      <LightningIcon size={13} weight="fill" className={live ? 'text-teal-bright' : 'text-text-dim'} />
      <span className={tone}>
        ${usage.remaining.toFixed(2)}
        {usage.limit ? <span className="text-text-dim">/${usage.limit.toFixed(0)}</span> : null}
      </span>
      {!live && <span className="text-[10px] text-text-dim">mock</span>}
    </span>
  )
}
