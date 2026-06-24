import { createBrowserRouter } from 'react-router-dom'
import { AuthPage } from './auth/AuthPage'
import { RequireAuth } from './auth/RequireAuth'
import { WorkspaceHub } from './app/WorkspaceHub'

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
])
