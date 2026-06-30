import { Suspense, lazy, type ReactNode } from 'react'
import { createBrowserRouter } from 'react-router-dom'
import { SpinnerGapIcon } from '@phosphor-icons/react'
import { RequireAuth } from './auth/RequireAuth'
import { HomeRoute } from './app/HomeRoute'

// Route-level code splitting — the unauthenticated landing/login path no longer
// ships the entire authenticated studio. HomeRoute stays eager (it's the entry).
const AuthPage = lazy(() => import('./auth/AuthPage').then((m) => ({ default: m.AuthPage })))
const ProjectWorkspace = lazy(() =>
  import('./app/ProjectWorkspace').then((m) => ({ default: m.ProjectWorkspace })),
)
const TeamPage = lazy(() => import('./app/TeamPage').then((m) => ({ default: m.TeamPage })))
const TrashPage = lazy(() => import('./app/TrashPage').then((m) => ({ default: m.TrashPage })))

function Lazy({ children }: { children: ReactNode }) {
  return (
    <Suspense
      fallback={
        <div className="grid h-[100dvh] place-items-center text-text-dim">
          <SpinnerGapIcon size={22} className="animate-spin" />
        </div>
      }
    >
      {children}
    </Suspense>
  )
}

export const router = createBrowserRouter([
  { path: '/login', element: <Lazy><AuthPage /></Lazy> },
  { path: '/', element: <HomeRoute /> },
  { path: '/team', element: <RequireAuth><Lazy><TeamPage /></Lazy></RequireAuth> },
  { path: '/trash', element: <RequireAuth><Lazy><TrashPage /></Lazy></RequireAuth> },
  {
    path: '/projects/:projectId',
    element: (
      <RequireAuth>
        <Lazy>
          <ProjectWorkspace />
        </Lazy>
      </RequireAuth>
    ),
  },
])
