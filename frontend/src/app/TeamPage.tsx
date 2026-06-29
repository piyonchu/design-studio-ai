import { useEffect, useState, type FormEvent } from 'react'
import { UserPlusIcon, XIcon, SpinnerGapIcon, CrownIcon } from '@phosphor-icons/react'
import * as api from '../lib/api'
import { ApiError } from '../lib/api'
import { AppShell } from './AppShell'
import { useAuth } from '../auth/AuthContext'

/**
 * Team page — manage who's in the current workspace. Owners can invite an
 * existing user by email and remove members; everyone can see the roster.
 * (No email-invite flow yet — the invitee must already have an account.)
 */
export function TeamPage() {
  const { user } = useAuth()
  const [ws, setWs] = useState<api.Workspace | null>(null)
  const [members, setMembers] = useState<api.WorkspaceMember[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [email, setEmail] = useState('')
  const [role, setRole] = useState<api.Role>('editor')
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    let active = true
    ;(async () => {
      try {
        const list = await api.listWorkspaces()
        const first = list[0] ?? null
        if (!active) return
        setWs(first)
        if (first) setMembers(await api.listMembers(first.id))
      } catch (e) {
        if (active) setError(e instanceof ApiError ? e.message : 'Failed to load team.')
      } finally {
        if (active) setLoading(false)
      }
    })()
    return () => {
      active = false
    }
  }, [])

  const myRole = members.find((m) => m.user_id === user?.id)?.role
  const isOwner = myRole === 'owner'

  async function invite(e: FormEvent) {
    e.preventDefault()
    if (!ws || !email.trim() || busy) return
    setBusy(true)
    setError(null)
    try {
      await api.inviteMember(ws.id, email.trim(), role)
      setMembers(await api.listMembers(ws.id))
      setEmail('')
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Invite failed.')
    } finally {
      setBusy(false)
    }
  }

  async function remove(userId: string) {
    if (!ws) return
    setError(null)
    try {
      await api.removeMember(ws.id, userId)
      setMembers(await api.listMembers(ws.id))
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Remove failed.')
    }
  }

  return (
    <AppShell>
      <div className="mx-auto max-w-[900px]">
        <div className="mb-6">
          <h1 className="text-2xl font-semibold tracking-tight text-text">Team</h1>
          <p className="mt-1 text-sm text-text-dim">
            {ws ? ws.name : 'Workspace'} · {members.length}{' '}
            {members.length === 1 ? 'member' : 'members'}
          </p>
        </div>

        {isOwner && (
          <form onSubmit={invite} className="glass mb-6 flex flex-wrap gap-2 rounded-[12px] p-2">
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Invite by email (must have an account)"
              className="min-w-0 flex-1 rounded-[8px] bg-surface-2/60 px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-2 focus:ring-teal/30"
            />
            <select
              value={role}
              onChange={(e) => setRole(e.target.value as api.Role)}
              className="rounded-[8px] bg-surface-2/60 px-3 py-2 text-sm text-text outline-none focus:ring-2 focus:ring-teal/30"
            >
              <option value="viewer">Viewer</option>
              <option value="editor">Editor</option>
              <option value="owner">Owner</option>
            </select>
            <button
              type="submit"
              disabled={busy || !email.trim()}
              className="inline-flex items-center gap-2 rounded-[8px] bg-teal px-4 py-2 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-50"
            >
              {busy ? <SpinnerGapIcon size={14} className="animate-spin" /> : <UserPlusIcon size={14} weight="bold" />}
              Invite
            </button>
          </form>
        )}

        {error && <p className="mb-3 text-sm text-rose-300">{error}</p>}

        {loading ? (
          <div className="grid place-items-center py-16 text-text-dim">
            <SpinnerGapIcon size={20} className="animate-spin" />
          </div>
        ) : (
          <ul className="space-y-2">
            {members.map((m) => {
              const name = api.userName(m)
              return (
                <li
                  key={m.user_id}
                  className="glass flex items-center gap-3 rounded-[12px] px-4 py-3"
                >
                  <span className="grid size-9 place-items-center rounded-full bg-indigo/25 text-sm font-semibold text-indigo-bright ring-1 ring-white/10">
                    {name[0]?.toUpperCase() ?? '?'}
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="flex items-center gap-1.5 truncate text-sm font-medium text-text">
                      {name}
                      {m.user_id === user?.id && <span className="text-xs text-text-dim">(you)</span>}
                    </p>
                    <p className="truncate text-xs text-text-dim">{m.email}</p>
                  </div>
                  <span
                    className={`inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium ${
                      m.role === 'owner'
                        ? 'bg-teal/15 text-teal-bright'
                        : 'bg-white/5 text-text-dim'
                    }`}
                  >
                    {m.role === 'owner' && <CrownIcon size={12} weight="fill" />}
                    {m.role}
                  </span>
                  {isOwner && m.user_id !== user?.id && (
                    <button
                      onClick={() => remove(m.user_id)}
                      aria-label={`Remove ${name}`}
                      className="grid size-7 place-items-center rounded-[8px] text-text-dim transition hover:bg-white/5 hover:text-rose-300"
                    >
                      <XIcon size={15} />
                    </button>
                  )}
                </li>
              )
            })}
          </ul>
        )}
      </div>
    </AppShell>
  )
}
