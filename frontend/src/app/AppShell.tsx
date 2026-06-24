import { useState, type ReactNode } from 'react'
import {
  HouseIcon,
  FoldersIcon,
  UsersThreeIcon,
  TrashIcon,
  SparkleIcon,
  MagnifyingGlassIcon,
  SignOutIcon,
} from '@phosphor-icons/react'
import { useAuth } from '../auth/AuthContext'

const NAV = [
  { icon: HouseIcon, label: 'Home', active: true },
  { icon: FoldersIcon, label: 'Projects', active: false },
  { icon: UsersThreeIcon, label: 'Team', active: false },
  { icon: TrashIcon, label: 'Trash', active: false },
]

export function AppShell({
  search,
  onSearch,
  children,
}: {
  search: string
  onSearch: (v: string) => void
  children: ReactNode
}) {
  const { user, logout } = useAuth()
  const [menuOpen, setMenuOpen] = useState(false)
  const initial = user?.email?.[0]?.toUpperCase() ?? '?'

  return (
    <div className="relative min-h-[100dvh]">
      <div className="app-aurora" />

      {/* Left icon rail */}
      <aside className="glass fixed inset-y-3 left-3 z-20 flex w-16 flex-col items-center justify-between rounded-[16px] py-5">
        <div className="flex flex-col items-center gap-1">
          <span className="mb-4 grid size-9 place-items-center rounded-[10px] bg-teal/15 text-teal-bright">
            <SparkleIcon size={20} weight="fill" />
          </span>
          {NAV.map(({ icon: Icon, label, active }) => (
            <button
              key={label}
              title={label}
              aria-label={label}
              className={`grid size-10 place-items-center rounded-[10px] transition ${
                active
                  ? 'bg-white/10 text-teal-bright'
                  : 'text-text-dim hover:bg-white/5 hover:text-text'
              }`}
            >
              <Icon size={20} weight={active ? 'fill' : 'regular'} />
            </button>
          ))}
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
          <div className="relative mx-auto w-full max-w-xl">
            <MagnifyingGlassIcon
              size={18}
              className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-text-dim"
            />
            <input
              value={search}
              onChange={(e) => onSearch(e.target.value)}
              placeholder="Search projects, flows, assets…"
              className="glass w-full rounded-full py-2.5 pl-11 pr-4 text-sm text-text outline-none transition placeholder:text-text-dim focus:ring-2 focus:ring-indigo/40"
            />
          </div>
          <div className="relative">
            <button
              onClick={() => setMenuOpen((v) => !v)}
              className="grid size-9 place-items-center rounded-full bg-indigo/25 text-sm font-semibold text-indigo-bright ring-1 ring-white/10"
              aria-label="Account menu"
            >
              {initial}
            </button>
            {menuOpen && (
              <div className="glass absolute right-0 top-11 z-30 w-52 rounded-[12px] p-1.5 text-sm">
                <p className="truncate px-3 py-2 text-xs text-text-dim">{user?.email}</p>
                <button
                  onClick={logout}
                  className="flex w-full items-center gap-2 rounded-[8px] px-3 py-2 text-left text-text transition hover:bg-white/5"
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
