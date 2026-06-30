import { useNavigate } from 'react-router-dom'
import { SparkleIcon, TrashIcon } from '@phosphor-icons/react'
import type { Project } from '../lib/api'
import { seededGradient } from '../lib/gradient'

export function ProjectCard({
  project,
  onDelete,
}: {
  project: Project
  onDelete?: (id: string) => void
}) {
  const navigate = useNavigate()
  const open = () => navigate(`/projects/${project.id}`)
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={open}
      onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && open()}
      className="glass group relative block w-full cursor-pointer overflow-hidden rounded-[16px] text-left transition duration-200 hover:-translate-y-0.5 hover:ring-1 hover:ring-teal/40"
    >
      {onDelete && (
        <button
          onClick={(e) => {
            e.stopPropagation()
            onDelete(project.id)
          }}
          aria-label={`Move ${project.name} to trash`}
          title="Move to trash"
          className="absolute right-2 top-2 z-10 grid size-8 place-items-center rounded-[8px] bg-black/40 text-text-dim opacity-0 backdrop-blur transition hover:text-rose-300 group-hover:opacity-100"
        >
          <TrashIcon size={15} />
        </button>
      )}
      {/* Seeded gradient tile (placeholder for rendered-asset previews). */}
      <div
        className="relative aspect-[4/3] w-full"
        style={{ backgroundImage: seededGradient(project.id) }}
      >
        <div className="absolute inset-0 bg-gradient-to-t from-black/40 to-transparent" />
      </div>

      <div className="p-4">
        <h3 className="truncate text-sm font-semibold text-text" title={project.name}>
          {project.name}
        </h3>
        <div className="mt-2 flex min-w-0 items-center gap-1.5">
          <span
            className="inline-flex max-w-full items-center gap-1 rounded-full bg-teal/15 px-2 py-0.5 text-[11px] font-medium text-teal-bright"
            title={project.brief ?? undefined}
          >
            <SparkleIcon size={11} weight="fill" className="shrink-0" />
            <span className="truncate">{project.brief ? project.brief : 'No brief yet'}</span>
          </span>
        </div>
      </div>
    </div>
  )
}
