import { useEffect, useState, type ComponentType } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import {
  ArrowLeftIcon,
  PaletteIcon,
  SquaresFourIcon,
  TrayIcon,
  TreeStructureIcon,
  StackIcon,
  ClockCounterClockwiseIcon,
  ShieldCheckIcon,
} from '@phosphor-icons/react'
import * as api from '../lib/api'
import { ApiError } from '../lib/api'
import { AssetLibrary } from './assets/AssetLibrary'
import { ReviewQueue } from './assets/ReviewQueue'
import { LineageView } from './assets/LineageView'
import { CanonView } from './canon/CanonView'
import { ContextAsk } from './canon/ContextAsk'
import { CollectionsView } from './collections/CollectionsView'
import { ActivityView } from './activity/ActivityView'
import { AccessView } from './AccessView'

type Tab = 'canon' | 'assets' | 'review' | 'lineage' | 'collections' | 'activity' | 'access'

const NAV: { id: Tab; label: string; icon: ComponentType<{ size?: number; weight?: 'fill' | 'regular' }> }[] = [
  { id: 'assets', label: 'Board', icon: SquaresFourIcon },
  { id: 'canon', label: 'Canon', icon: PaletteIcon },
  { id: 'review', label: 'Review', icon: TrayIcon },
  { id: 'lineage', label: 'Lineage', icon: TreeStructureIcon },
  { id: 'collections', label: 'Collections', icon: StackIcon },
  { id: 'activity', label: 'Activity', icon: ClockCounterClockwiseIcon },
  { id: 'access', label: 'Access', icon: ShieldCheckIcon },
]

/**
 * Project view — the asset studio shell. A left rail switches between the canon,
 * asset board, review queue, lineage, and collections views.
 */
export function ProjectWorkspace() {
  const { projectId } = useParams<{ projectId: string }>()
  const navigate = useNavigate()

  const [project, setProject] = useState<api.Project | null>(null)
  const [tab, setTab] = useState<Tab>('assets')
  const [access, setAccess] = useState<api.ProjectAccess | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!projectId) return
    let alive = true
    api
      .getProject(projectId)
      .then((p) => {
        if (alive) setProject(p)
      })
      .catch((err) => {
        if (alive) setError(err instanceof ApiError ? err.message : 'Failed to load project.')
      })
    api
      .getProjectAccess(projectId)
      .then((a) => alive && setAccess(a))
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [projectId])

  // Until access resolves, don't disable approval (the backend still enforces).
  const canApprove = access ? access.can_approve : true

  return (
    <div className="relative flex h-[100dvh]">
      <div className="app-aurora" />

      {/* Left rail */}
      <aside className="relative z-20 flex w-48 shrink-0 flex-col gap-1 px-3 py-3">
        <div className="flex items-center gap-2 px-1 py-2">
          <button
            onClick={() => navigate('/')}
            aria-label="Back to workspace"
            className="grid size-8 shrink-0 place-items-center rounded-[10px] text-text-dim transition hover:bg-white/5 hover:text-text"
          >
            <ArrowLeftIcon size={18} />
          </button>
          <p className="truncate text-sm font-semibold text-text" title={project?.name ?? undefined}>
            {project?.name ?? 'Project'}
          </p>
        </div>

        <nav className="mt-1 flex flex-col gap-0.5">
          {NAV.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              className={`flex items-center gap-2.5 rounded-[10px] px-3 py-2 text-sm font-medium transition ${
                tab === id ? 'bg-teal/15 text-teal-bright' : 'text-text-dim hover:bg-white/5 hover:text-text'
              }`}
            >
              <Icon size={17} weight={tab === id ? 'fill' : 'regular'} />
              {label}
            </button>
          ))}
        </nav>
      </aside>

      {/* Content */}
      <div className="relative z-10 flex min-h-0 flex-1 flex-col py-3 pr-3">
        {error && (
          <p className="mb-2 rounded-[10px] border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-sm text-rose-300">
            {error}
          </p>
        )}
        <div className="flex min-h-0 flex-1">
          {projectId &&
            (tab === 'canon' ? (
              <div className="flex min-h-0 flex-1 flex-col gap-3">
                <ContextAsk projectId={projectId} />
                <CanonView projectId={projectId} vertical={project?.vertical} />
              </div>
            ) : tab === 'review' ? (
              <ReviewQueue projectId={projectId} canApprove={canApprove} />
            ) : tab === 'lineage' ? (
              <LineageView projectId={projectId} />
            ) : tab === 'collections' ? (
              <CollectionsView projectId={projectId} vertical={project?.vertical} />
            ) : tab === 'activity' ? (
              <ActivityView projectId={projectId} />
            ) : tab === 'access' ? (
              <AccessView projectId={projectId} />
            ) : (
              <AssetLibrary projectId={projectId} vertical={project?.vertical} canApprove={canApprove} />
            ))}
        </div>
      </div>
    </div>
  )
}
