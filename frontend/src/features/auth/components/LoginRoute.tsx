import { Navigate, useLocation } from 'react-router-dom'
import type { ReactNode } from 'react'
import { SessionBootstrapScreen } from './SessionBootstrapScreen'
import { postLoginDestination } from '../model/redirectDestination'
import { useAuthSession } from '../model/useAuthSession'

export function LoginRoute({ children }: { children: ReactNode }) {
  const { status } = useAuthSession()
  const location = useLocation()

  if (status === 'BOOTSTRAPPING') return <SessionBootstrapScreen />
  if (status === 'AUTHENTICATED') {
    return <Navigate to={postLoginDestination(location.state)} replace />
  }
  return children
}
