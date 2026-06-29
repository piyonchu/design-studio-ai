import { useEffect, useState } from 'react'
import { ShieldCheckIcon, SpinnerGapIcon, CrownIcon } from '@phosphor-icons/react'
import * as api from '../lib/api'
import { ApiError } from '../lib/api'

const ROLES: api.ProjectRole[] = ['viewer', 'editor', 'reviewer', 'owner']
const ROLE_HINT: Record<api.ProjectRole, string> = {
  viewer: 'Can view assets',
  editor: 'Can generate, edit, submit for review',
  reviewer: 'Editor + can approve assets',
  owner: 'Full control + manage access',
}

/**
 * Per-project access (Phase C). Lists workspace members with the role they
 * effectively have here; a project owner can override any member's role —
 * notably granting **reviewer** (who may approve, the review gate). Workspace
 * owners are always project owners and can't be downgraded.
 */
export function AccessView({ projectId }: { projectId: string }) {
  const [members, setMembers] = useState<api.ProjectMemberRow[]>([])
  const [access, setAccess] = useState<api.ProjectAccess | null>(null)
  const [loading, setLoading] = useState(true)
  const [busyUser, setBusyUser] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  function load() {
    Promise.all([api.listProjectMembers(projectId), api.getProjectAccess(projectId)])
      .then(([m, a]) => {
        setMembers(m)
        setAccess(a)
      })
      .catch((e) => setError(e instanceof ApiError ? e.message : 'Failed to load access.'))
      .finally(() => setLoading(false))
  }
  useEffect(load, [projectId])

  const canManage = access?.role === 'owner'

  async function change(m: api.ProjectMemberRow, role: api.ProjectRole | '') {
    setBusyUser(m.user_id)
    setError(null)
    try {
      if (role === '') await api.clearProjectRole(projectId, m.user_id)
      else await api.setProjectRole(projectId, m.user_id, role)
      load()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Update failed.')
    } finally {
      setBusyUser(null)
    }
  }

  return (
    <div className="glass flex min-h-0 flex-1 flex-col overflow-hidden rounded-[16px]">
      <div className="flex items-center gap-2 border-b border-white/8 px-5 py-4">
        <span className="grid size-7 place-items-center rounded-[8px] bg-indigo-400/15 text-indigo-300">
          <ShieldCheckIcon size={15} weight="fill" />
        </span>
        <p className="text-sm font-medium text-text">Project Access</p>
        <span className="text-sm text-text-dim">· {members.length}</span>
        {!canManage && !loading && (
          <span className="ml-auto text-xs text-text-dim">View only — ask a project owner to change roles</span>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {loading ? (
          <p className="py-12 text-center text-sm text-text-dim">Loading…</p>
        ) : (
          <>
            <p className="mb-3 text-xs text-text-dim">
              Roles layer on workspace membership. A <span className="text-teal-bright">reviewer</span> may approve
              assets; an editor can submit for review but not self-approve.
            </p>
            {error && <p className="mb-3 text-xs text-rose-300">{error}</p>}
            <ul className="space-y-1.5">
              {members.map((m) => {
                const isWsOwner = m.workspace_role === 'owner'
                return (
                  <li
                    key={m.user_id}
                    className="flex items-center gap-3 rounded-[10px] border border-white/8 bg-surface-2/40 px-3 py-2"
                  >
                    <span className="grid size-8 shrink-0 place-items-center rounded-full bg-white/8 text-xs font-semibold text-text">
                      {(m.display_name || m.email).charAt(0).toUpperCase()}
                    </span>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm text-text">{m.display_name || m.email}</p>
                      <p className="truncate text-[11px] text-text-dim">
                        {m.email} · workspace {m.workspace_role}
                        {m.overridden && <span className="text-indigo-300"> · override</span>}
                      </p>
                    </div>

                    {isWsOwner ? (
                      <span
                        className="inline-flex items-center gap-1 rounded-[8px] bg-amber-400/15 px-2.5 py-1.5 text-xs text-amber-200"
                        title="Workspace owners always have full project access"
                      >
                        <CrownIcon size={13} weight="fill" />
                        owner
                      </span>
                    ) : busyUser === m.user_id ? (
                      <SpinnerGapIcon size={16} className="animate-spin text-text-dim" />
                    ) : canManage ? (
                      <select
                        value={m.project_role}
                        onChange={(e) => change(m, e.target.value as api.ProjectRole)}
                        className="rounded-[8px] bg-surface/70 px-2.5 py-1.5 text-xs text-text outline-none focus:ring-1 focus:ring-teal/40"
                        title={ROLE_HINT[m.project_role]}
                      >
                        {ROLES.map((r) => (
                          <option key={r} value={r}>
                            {r}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <span className="rounded-[8px] bg-white/8 px-2.5 py-1.5 text-xs text-text-dim">
                        {m.project_role}
                      </span>
                    )}
                  </li>
                )
              })}
            </ul>
          </>
        )}
      </div>
    </div>
  )
}
