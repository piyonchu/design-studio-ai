import { createBrowserRouter } from 'react-router-dom'
import { AuthPage } from './auth/AuthPage'
import { RequireAuth } from './auth/RequireAuth'
import { WorkspaceHub } from './app/WorkspaceHub'
import { ProjectWorkspace } from './app/ProjectWorkspace'

export const router = createBrowserRouter([
  { path: '/login', element: <AuthPage /> },
  {
    path: '/',
    element: (
      <RequireAuth>
        <WorkspaceHub />
      </RequireAuth>
    ),
  },
  {
    path: '/projects/:projectId',
    element: (
      <RequireAuth>
        <ProjectWorkspace />
      </RequireAuth>
    ),
  },
])
