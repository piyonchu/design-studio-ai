import { useEffect, useState } from 'react'
import { SpinnerGapIcon } from '@phosphor-icons/react'
import * as api from '../../lib/api'

/**
 * Ambient indicator of in-flight generation jobs for a project — polls the
 * job list and shows queued/running counts. The board's own generate flow
 * refreshes assets on completion; this just surfaces that work is happening
 * (incl. jobs started elsewhere). Renders nothing when the queue is idle.
 */
export function JobsBanner({ projectId }: { projectId: string }) {
  const [active, setActive] = useState<api.Job[]>([])

  useEffect(() => {
    let alive = true
    let timer: ReturnType<typeof setTimeout>
    const poll = async () => {
      try {
        const jobs = await api.listJobs(projectId)
        if (!alive) return
        setActive(jobs.filter((j) => j.status === 'queued' || j.status === 'running'))
      } catch {
        /* ambient — ignore */
      }
      if (alive) timer = setTimeout(poll, 1500)
    }
    poll()
    return () => {
      alive = false
      clearTimeout(timer)
    }
  }, [projectId])

  if (active.length === 0) return null
  const running = active.filter((j) => j.status === 'running').length

  return (
    <div className="mb-3 flex items-center gap-2 rounded-[10px] border border-teal/20 bg-teal/8 px-3 py-2 text-xs text-teal-bright">
      <SpinnerGapIcon size={14} className="animate-spin" />
      <span>
        Generating assets — {active.length} job{active.length > 1 ? 's' : ''} queued
        {running > 0 ? ` · ${running} running now` : ''}
      </span>
    </div>
  )
}
