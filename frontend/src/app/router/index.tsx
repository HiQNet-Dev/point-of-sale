import { Navigate, createBrowserRouter } from 'react-router-dom'
import { LoginRoute } from '../../features/auth/components/LoginRoute'
import { ProtectedRoute } from '../../features/auth/components/ProtectedRoute'
import { LoginPage } from '../../features/auth/pages/LoginPage'
import { SessionHomePage } from '../../features/auth/pages/SessionHomePage'

/**
 * Public login and one protected session landing page. Business modules add
 * their own protected routes as they are implemented.
 */
export const router = createBrowserRouter([
  {
    path: '/login',
    element: (
      <LoginRoute>
        <LoginPage />
      </LoginRoute>
    ),
  },
  {
    path: '/',
    element: (
      <ProtectedRoute>
        <SessionHomePage />
      </ProtectedRoute>
    ),
  },
  {
    path: '*',
    element: <Navigate to="/" replace />,
  },
])
