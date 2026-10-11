import { Navigate, useLocation } from 'react-router-dom'
import type { ReactNode } from 'react'
import { SessionBootstrapScreen } from './SessionBootstrapScreen'
import { useAuthSession } from '../model/useAuthSession'

export function ProtectedRoute({ children }: { children: ReactNode }) {
  const { status } = useAuthSession()
  const location = useLocation()

  if (status === 'BOOTSTRAPPING') return <SessionBootstrapScreen />
  if (status === 'UNAUTHENTICATED') {
    return <Navigate to="/login" replace state={{ from: location }} />
  }
  return children
}
