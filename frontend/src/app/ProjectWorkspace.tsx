import { Suspense, lazy, useEffect, useState, type ComponentType } from 'react'
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
  SpinnerGapIcon,
} from '@phosphor-icons/react'
import * as api from '../lib/api'
import { formatApiError } from '../lib/api'
import { AssetLibrary } from './assets/AssetLibrary'
import { ErrorBanner } from './ui/ErrorBanner'

// The Board is the default tab → eager. The rest split into their own chunks,
// loaded the first time their tab is opened.
const ReviewQueue = lazy(() => import('./assets/ReviewQueue').then((m) => ({ default: m.ReviewQueue })))
const LineageView = lazy(() => import('./assets/LineageView').then((m) => ({ default: m.LineageView })))
const CanonView = lazy(() => import('./canon/CanonView').then((m) => ({ default: m.CanonView })))
const ContextAsk = lazy(() => import('./canon/ContextAsk').then((m) => ({ default: m.ContextAsk })))
const CollectionsView = lazy(() =>
  import('./collections/CollectionsView').then((m) => ({ default: m.CollectionsView })),
)
const ActivityView = lazy(() => import('./activity/ActivityView').then((m) => ({ default: m.ActivityView })))
const AccessView = lazy(() => import('./AccessView').then((m) => ({ default: m.AccessView })))

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
  const [reloadTick, setReloadTick] = useState(0)

  useEffect(() => {
    if (!projectId) return
    let alive = true
    setError(null)
    api
      .getProject(projectId)
      .then((p) => {
        if (alive) setProject(p)
      })
      .catch((err) => {
        if (alive) setError(formatApiError(err, "Couldn't load this project. It may have been deleted or you may not have access."))
      })
    api
      .getProjectAccess(projectId)
      .then((a) => alive && setAccess(a))
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [projectId, reloadTick])

  // Until access resolves, don't disable approval (the backend still enforces).
  const canApprove = access ? access.can_approve : true

  return (
    <div className="relative flex h-[100dvh]">
      <div className="app-aurora" />

      {/* Left rail — icon-only below lg, full labels on desktop */}
      <aside className="relative z-20 flex w-14 shrink-0 flex-col border-r border-white/6 px-2 py-3 lg:w-48 lg:px-3">
        <div className="flex items-center justify-center gap-2 px-1 pb-2 lg:justify-start">
          <button
            onClick={() => navigate('/')}
            aria-label="Back to workspace"
            className="icon-btn size-8 shrink-0"
          >
            <ArrowLeftIcon size={18} />
          </button>
          <p className="hidden truncate text-sm font-semibold text-text lg:block" title={project?.name ?? undefined}>
            {project?.name ?? 'Project'}
          </p>
        </div>

        <nav className="mt-2 flex flex-col gap-0.5 border-t border-white/6 pt-3">
          {NAV.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              title={label}
              className={`flex items-center justify-center gap-2.5 rounded-[10px] px-0 py-2 text-sm font-medium transition duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo/40 lg:justify-start lg:px-3 ${
              tab === id
                ? 'bg-teal/15 text-teal-bright ring-1 ring-teal/20'
                : id === 'review'
                  ? 'text-text-dim hover:bg-warning/8 hover:text-warning'
                  : 'text-text-dim hover:bg-white/5 hover:text-text'
              }`}
            >
              <Icon size={17} weight={tab === id ? 'fill' : 'regular'} />
              <span className="hidden lg:inline">{label}</span>
            </button>
          ))}
        </nav>
      </aside>

      {/* Content */}
      <div className="relative z-10 flex min-h-0 flex-1 flex-col py-3 pr-3">
        {error && (
          <ErrorBanner
            message={error}
            onRetry={() => setReloadTick((n) => n + 1)}
            onDismiss={() => setError(null)}
            className="mb-2"
          />
        )}
        <div className="tab-fill">
          <Suspense
            fallback={
              <div className="flex min-h-0 flex-1 items-center justify-center text-text-dim">
                <SpinnerGapIcon size={20} className="animate-spin" />
              </div>
            }
          >
            {projectId &&
              (tab === 'canon' ? (
                <div className="tab-fill gap-4">
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
          </Suspense>
        </div>
      </div>
    </div>
  )
}
