import { LandingPage } from './LandingPage'
import { WorkspaceHub } from './WorkspaceHub'
import { useAuth } from '../auth/AuthContext'

export function HomeRoute() {
  const { user } = useAuth()

  return user ? <WorkspaceHub /> : <LandingPage />
}
