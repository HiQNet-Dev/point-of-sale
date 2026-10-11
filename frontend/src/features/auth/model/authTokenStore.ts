import type { TokenResponse } from '../api/authApi'

export interface AuthTokens {
  accessToken: string
  refreshToken: string
}

export interface AuthTokenSnapshot {
  tokens: AuthTokens | null
  revision: number
  sessionId: number | null
}

let tokens: AuthTokens | null = null
let revision = 0
let sessionId: number | null = null

export const authTokenStore = {
  snapshot(): AuthTokenSnapshot {
    return { tokens: tokens ? { ...tokens } : null, revision, sessionId }
  },

  getAccessToken(): string | null {
    return tokens?.accessToken ?? null
  },

  getRefreshToken(): string | null {
    return tokens?.refreshToken ?? null
  },

  getSessionId(): number | null {
    return sessionId
  },

  set(response: TokenResponse): void {
    tokens = { accessToken: response.accessToken, refreshToken: response.refreshToken }
    revision += 1
    sessionId = revision
  },

  rotate(response: TokenResponse, expectedRevision: number): boolean {
    if (revision !== expectedRevision || sessionId === null) return false
    tokens = { accessToken: response.accessToken, refreshToken: response.refreshToken }
    revision += 1
    return true
  },

  clear(): void {
    tokens = null
    revision += 1
    sessionId = null
  },

  isCurrent(expectedRevision: number): boolean {
    return revision === expectedRevision
  },
}
