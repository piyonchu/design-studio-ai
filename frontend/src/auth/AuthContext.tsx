import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import * as api from '../lib/api'

interface AuthState {
  user: api.User | null
  loading: boolean
  login: (email: string, password: string) => Promise<void>
  signup: (email: string, password: string) => Promise<void>
  logout: () => Promise<void>
}

const AuthCtx = createContext<AuthState | null>(null)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<api.User | null>(null)
  const [loading, setLoading] = useState(true)

  // Resume session on load via the cookie.
  useEffect(() => {
    api
      .me()
      .then(setUser)
      .catch(() => setUser(null))
      .finally(() => setLoading(false))
  }, [])

  const login = async (email: string, password: string) => {
    setUser(await api.login(email, password))
  }
  const signup = async (email: string, password: string) => {
    const { user } = await api.signup(email, password)
    setUser(user)
  }
  const logout = async () => {
    await api.logout()
    setUser(null)
  }

  return (
    <AuthCtx.Provider value={{ user, loading, login, signup, logout }}>
      {children}
    </AuthCtx.Provider>
  )
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthCtx)
  if (!ctx) throw new Error('useAuth must be used within AuthProvider')
  return ctx
}
