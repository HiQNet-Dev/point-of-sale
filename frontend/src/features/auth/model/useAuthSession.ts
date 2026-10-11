import { useContext } from 'react'
import { AuthSessionContext } from './authSessionContext'

export function useAuthSession() {
  const session = useContext(AuthSessionContext)
  if (!session) throw new Error('useAuthSession must be used inside AuthSessionProvider')
  return session
}
