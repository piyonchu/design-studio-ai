import { SparkleIcon } from '@phosphor-icons/react'
import type { Project } from '../lib/api'
import { seededGradient } from '../lib/gradient'

export function ProjectCard({ project }: { project: Project }) {
  return (
    <button className="glass group block w-full overflow-hidden rounded-[16px] text-left transition duration-200 hover:-translate-y-0.5 hover:ring-1 hover:ring-teal/40">
      {/* Seeded gradient tile (placeholder for rendered-DSL previews). */}
      <div
        className="relative aspect-[4/3] w-full"
        style={{ backgroundImage: seededGradient(project.id) }}
      >
        <div className="absolute inset-0 bg-gradient-to-t from-black/40 to-transparent" />
      </div>

      <div className="p-4">
        <h3 className="truncate text-sm font-semibold text-text">{project.name}</h3>
        <div className="mt-2 flex items-center gap-1.5">
          <span className="inline-flex items-center gap-1 rounded-full bg-teal/15 px-2 py-0.5 text-[11px] font-medium text-teal-bright">
            <SparkleIcon size={11} weight="fill" />
            {project.brief ? project.brief : 'No AI updates yet'}
          </span>
        </div>
      </div>
    </button>
  )
}
