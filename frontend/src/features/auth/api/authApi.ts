import { requestJson } from '../../../shared/api/client'

export interface LoginRequest {
  organizationCode: string
  email: string
  password: string
}

export interface TokenResponse {
  accessToken: string
  tokenType: 'Bearer'
  expiresIn: number
  refreshToken: string
  refreshExpiresAt: string
}

export interface AuthMe {
  userId: string
  organizationId: string
  email: string
  status: string
  organization: {
    currency: string
    locale: string
    timezone: string
  }
}

export function login(request: LoginRequest): Promise<TokenResponse> {
  return requestJson<TokenResponse>(
    '/auth/login',
    { method: 'POST', body: JSON.stringify(request) },
    { authenticated: false, retryOnUnauthorized: false },
  )
}

export function refresh(refreshToken: string): Promise<TokenResponse> {
  return requestJson<TokenResponse>(
    '/auth/refresh',
    { method: 'POST', body: JSON.stringify({ refreshToken }) },
    { authenticated: false, retryOnUnauthorized: false },
  )
}

export function logout(refreshToken: string): Promise<void> {
  return requestJson<void>(
    '/auth/logout',
    { method: 'POST', body: JSON.stringify({ refreshToken }) },
    { authenticated: false, retryOnUnauthorized: false },
  )
}

export function getMe(): Promise<AuthMe> {
  return requestJson<AuthMe>('/auth/me')
}
