import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { ArrowLeftIcon } from '@phosphor-icons/react'
import * as api from '../lib/api'
import { ApiError } from '../lib/api'
import { AssetLibrary } from './assets/AssetLibrary'
import { ReviewQueue } from './assets/ReviewQueue'
import { CanonView } from './canon/CanonView'
import { CollectionsView } from './collections/CollectionsView'

type Tab = 'canon' | 'assets' | 'review' | 'collections'

/**
 * Project view — the asset studio for one project. Currently the asset library;
 * the full CanonForge board (canon, derive, review, collections, export) builds
 * on top of this per PLAN.md.
 */
export function ProjectWorkspace() {
  const { projectId } = useParams<{ projectId: string }>()
  const navigate = useNavigate()

  const [project, setProject] = useState<api.Project | null>(null)
  const [tab, setTab] = useState<Tab>('canon')
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
    return () => {
      alive = false
    }
  }, [projectId])

  return (
    <div className="relative flex h-[100dvh] flex-col">
      <div className="app-aurora" />

      <header className="relative z-20 flex items-center gap-3 px-4 py-3">
        <button
          onClick={() => navigate('/')}
          aria-label="Back to workspace"
          className="grid size-9 place-items-center rounded-[10px] text-text-dim transition hover:bg-white/5 hover:text-text"
        >
          <ArrowLeftIcon size={18} />
        </button>
        <p className="shrink-0 text-sm font-semibold text-text">{project?.name ?? 'Project'}</p>

        <nav className="ml-2 flex items-center gap-1 rounded-[10px] bg-surface-2/50 p-1">
          {(['canon', 'assets', 'review', 'collections'] as Tab[]).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`rounded-[8px] px-3 py-1.5 text-sm font-medium capitalize transition ${
                tab === t ? 'bg-teal text-bg' : 'text-text-dim hover:text-text'
              }`}
            >
              {t}
            </button>
          ))}
        </nav>
      </header>

      {error && (
        <p className="relative z-10 mx-4 mb-2 rounded-[10px] border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-sm text-rose-300">
          {error}
        </p>
      )}

      <div className="relative z-10 flex min-h-0 flex-1 px-3 pb-3">
        {projectId &&
          (tab === 'canon' ? (
            <CanonView projectId={projectId} />
          ) : tab === 'review' ? (
            <ReviewQueue projectId={projectId} />
          ) : tab === 'collections' ? (
            <CollectionsView projectId={projectId} />
          ) : (
            <AssetLibrary projectId={projectId} />
          ))}
      </div>
    </div>
  )
}
