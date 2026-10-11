import { createContext } from 'react'
import type { AuthMe, LoginRequest } from '../api/authApi'

export type AuthSessionStatus = 'BOOTSTRAPPING' | 'AUTHENTICATED' | 'UNAUTHENTICATED'

export interface AuthSessionValue {
  status: AuthSessionStatus
  me: AuthMe | null
  logoutNotice: string | null
  login(request: LoginRequest): Promise<AuthMe>
  logout(): Promise<boolean>
}

export const AuthSessionContext = createContext<AuthSessionValue | null>(null)
