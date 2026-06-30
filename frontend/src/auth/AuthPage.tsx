import { useEffect, useState, type FormEvent } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { SparkleIcon, SpinnerGapIcon } from '@phosphor-icons/react'
import { useAuth } from './AuthContext'
import { formatApiError } from '../lib/api'

type Mode = 'login' | 'signup'

function modeFromParams(params: URLSearchParams): Mode {
  return params.get('mode') === 'login' ? 'login' : 'signup'
}

export function AuthPage() {
  const { login, signup } = useAuth()
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const [mode, setMode] = useState<Mode>(() => modeFromParams(searchParams))
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    setMode(modeFromParams(searchParams))
  }, [searchParams])

  function switchMode(next: Mode) {
    setMode(next)
    setSearchParams({ mode: next })
    setError(null)
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault()
    setError(null)
    if (mode === 'signup' && password.length < 8) {
      setError('Use at least 8 characters for your password.')
      return
    }
    setBusy(true)
    try {
      if (mode === 'login') await login(email, password)
      else await signup(email, password)
      navigate('/', { replace: true })
    } catch (err) {
      setError(formatApiError(err, "Couldn't sign you in. Check your email and password, then try again."))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="relative grid min-h-[100dvh] place-items-center px-4">
      <div className="app-aurora" />
      <div className="glass relative z-10 w-full max-w-md rounded-[16px] p-8">
        <div className="mb-7 flex items-center gap-2.5">
          <span className="grid size-9 place-items-center rounded-[10px] bg-teal/15 text-teal-bright">
            <SparkleIcon size={20} weight="fill" />
          </span>
          <div className="leading-tight">
            <p className="text-sm font-medium text-text">CanonForge</p>
            <p className="text-xs text-text-dim">
              {mode === 'login' ? 'Welcome back' : 'Create your workspace'}
            </p>
          </div>
        </div>

        <form onSubmit={onSubmit} className="grid gap-4">
          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-text-muted">Email</span>
            <input
              type="email"
              required
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="rounded-[10px] border border-border bg-surface-2/60 px-3 py-2.5 text-sm text-text outline-none transition focus:border-teal/60 focus:ring-2 focus:ring-teal/20"
              placeholder="you@studio.com"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-text-muted">Password</span>
            <input
              type="password"
              required
              autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="rounded-[10px] border border-border bg-surface-2/60 px-3 py-2.5 text-sm text-text outline-none transition focus:border-teal/60 focus:ring-2 focus:ring-teal/20"
              placeholder={mode === 'signup' ? 'At least 8 characters' : '••••••••'}
            />
          </label>

          {error && (
            <p className="rounded-[10px] border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-xs text-rose-300">
              {error}
            </p>
          )}

          <button
            type="submit"
            disabled={busy}
            className="mt-1 inline-flex items-center justify-center gap-2 rounded-[10px] bg-teal px-4 py-2.5 text-sm font-semibold text-bg transition active:translate-y-px disabled:opacity-60"
          >
            {busy && <SpinnerGapIcon size={16} className="animate-spin" />}
            {mode === 'login' ? 'Sign in' : 'Create account'}
          </button>
        </form>

        <p className="mt-6 text-center text-xs text-text-dim">
          {mode === 'login' ? "Don't have an account?" : 'Already have an account?'}{' '}
          <button
            type="button"
            onClick={() => switchMode(mode === 'login' ? 'signup' : 'login')}
            className="font-medium text-teal-bright hover:underline"
          >
            {mode === 'login' ? 'Sign up' : 'Sign in'}
          </button>
        </p>
      </div>
    </div>
  )
}
