import { useState, type ReactNode } from 'react'
import { Link, useLocation } from 'react-router-dom'
import {
  HouseIcon,
  UsersThreeIcon,
  TrashIcon,
  SparkleIcon,
  MagnifyingGlassIcon,
  SignOutIcon,
  CheckIcon,
} from '@phosphor-icons/react'
import * as api from '../lib/api'
import { useAuth } from '../auth/AuthContext'

const NAV = [
  { icon: HouseIcon, label: 'Home', to: '/' },
  { icon: UsersThreeIcon, label: 'Team', to: '/team' },
  { icon: TrashIcon, label: 'Trash', to: '/trash' },
]

export function AppShell({
  search,
  onSearch,
  children,
}: {
  search?: string
  onSearch?: (v: string) => void
  children: ReactNode
}) {
  const { user, logout, updateProfile } = useAuth()
  const { pathname } = useLocation()
  const [menuOpen, setMenuOpen] = useState(false)
  const [name, setName] = useState('')
  const [saving, setSaving] = useState(false)
  const label = user ? api.userName(user) : ''
  const initial = label[0]?.toUpperCase() ?? '?'

  async function saveName() {
    const n = name.trim()
    if (!n || saving) return
    setSaving(true)
    try {
      await updateProfile(n)
      setMenuOpen(false)
      setName('')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="relative min-h-[100dvh]">
      <div className="app-aurora" />

      {/* Left icon rail */}
      <aside className="glass fixed inset-y-3 left-3 z-20 flex w-16 flex-col items-center justify-between rounded-[16px] py-5">
        <div className="flex flex-col items-center gap-1">
          <Link
            to="/"
            className="mb-4 grid size-9 place-items-center rounded-[10px] bg-teal/15 text-teal-bright"
            aria-label="CanonForge home"
          >
            <SparkleIcon size={20} weight="fill" />
          </Link>
          {NAV.map(({ icon: Icon, label, to }) => {
            const active = to === '/' ? pathname === '/' : pathname.startsWith(to)
            return (
              <Link
                key={label}
                to={to}
                title={label}
                aria-label={label}
                aria-current={active ? 'page' : undefined}
                className={`grid size-10 place-items-center rounded-[10px] transition ${
                  active
                    ? 'bg-white/10 text-teal-bright'
                    : 'text-text-dim hover:bg-white/5 hover:text-text'
                }`}
              >
                <Icon size={20} weight={active ? 'fill' : 'regular'} />
              </Link>
            )
          })}
        </div>
        <button
          onClick={logout}
          title="Sign out"
          aria-label="Sign out"
          className="grid size-10 place-items-center rounded-[10px] text-text-dim transition hover:bg-white/5 hover:text-text"
        >
          <SignOutIcon size={20} />
        </button>
      </aside>

      {/* Main column */}
      <div className="relative z-10 pl-[5.5rem] pr-4">
        {/* Top bar */}
        <header className="flex items-center gap-4 py-4">
          {onSearch ? (
            <div className="relative mx-auto w-full max-w-xl">
              <MagnifyingGlassIcon
                size={18}
                className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-text-dim"
              />
              <input
                value={search ?? ''}
                onChange={(e) => onSearch(e.target.value)}
                placeholder="Search projects, flows, assets…"
                className="glass w-full rounded-full py-2.5 pl-11 pr-4 text-sm text-text outline-none transition placeholder:text-text-dim focus:ring-2 focus:ring-indigo/40"
              />
            </div>
          ) : (
            <div className="flex-1" />
          )}
          <div className="relative">
            <button
              onClick={() => setMenuOpen((v) => !v)}
              className="grid size-9 place-items-center rounded-full bg-indigo/25 text-sm font-semibold text-indigo-bright ring-1 ring-white/10"
              aria-label="Account menu"
            >
              {initial}
            </button>
            {menuOpen && (
              <div className="glass absolute right-0 top-11 z-30 w-64 rounded-[12px] p-2 text-sm">
                <p className="truncate px-2 py-1 font-medium text-text">{label}</p>
                <p className="truncate px-2 pb-2 text-xs text-text-dim">{user?.email}</p>
                <div className="flex items-center gap-1.5 border-t border-white/8 px-2 pb-1 pt-2">
                  <input
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && saveName()}
                    placeholder="Set display name…"
                    className="min-w-0 flex-1 rounded-[8px] bg-surface-2/60 px-2.5 py-1.5 text-xs text-text outline-none placeholder:text-text-dim focus:ring-2 focus:ring-teal/30"
                  />
                  <button
                    onClick={saveName}
                    disabled={!name.trim() || saving}
                    aria-label="Save display name"
                    className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-teal text-bg transition disabled:opacity-40"
                  >
                    <CheckIcon size={14} weight="bold" />
                  </button>
                </div>
                <button
                  onClick={logout}
                  className="mt-1 flex w-full items-center gap-2 rounded-[8px] px-2 py-2 text-left text-text transition hover:bg-white/5"
                >
                  <SignOutIcon size={16} /> Sign out
                </button>
              </div>
            )}
          </div>
        </header>

        <main className="pb-16">{children}</main>
      </div>
    </div>
  )
}
