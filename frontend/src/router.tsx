import { createBrowserRouter } from 'react-router-dom'
import { AuthPage } from './auth/AuthPage'
import { RequireAuth } from './auth/RequireAuth'
import { HomeRoute } from './app/HomeRoute'
import { ProjectWorkspace } from './app/ProjectWorkspace'
import { TeamPage } from './app/TeamPage'
import { TrashPage } from './app/TrashPage'

export const router = createBrowserRouter([
  { path: '/login', element: <AuthPage /> },
  { path: '/', element: <HomeRoute /> },
  { path: '/team', element: <RequireAuth><TeamPage /></RequireAuth> },
  { path: '/trash', element: <RequireAuth><TrashPage /></RequireAuth> },
  {
    path: '/projects/:projectId',
    element: (
      <RequireAuth>
        <ProjectWorkspace />
      </RequireAuth>
    ),
  },
])
