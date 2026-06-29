import { createBrowserRouter } from 'react-router-dom'
import { AuthPage } from './auth/AuthPage'
import { RequireAuth } from './auth/RequireAuth'
import { HomeRoute } from './app/HomeRoute'
import { ProjectWorkspace } from './app/ProjectWorkspace'

export const router = createBrowserRouter([
  { path: '/login', element: <AuthPage /> },
  { path: '/', element: <HomeRoute /> },
  {
    path: '/projects/:projectId',
    element: (
      <RequireAuth>
        <ProjectWorkspace />
      </RequireAuth>
    ),
  },
])
