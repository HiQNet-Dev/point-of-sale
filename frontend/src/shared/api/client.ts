import { API_BASE_URL } from '../../app/config/env'

/**
 * Shared API client foundation. Authenticated features use this client rather
 * than constructing Authorization headers or implementing refresh locally.
 */
export interface ApiErrorBody {
  timestamp?: string
  status?: number
  code?: string
  message?: string
  path?: string
  details?: string[]
}

export interface ApiAuthAdapter {
  getAccessToken(): string | null
  getSessionId?(): number | null
  refreshAccessToken(): Promise<string>
  onAuthorizationFailure?(): void
}

export interface RequestJsonOptions {
  authenticated?: boolean
  retryOnUnauthorized?: boolean
}

let authAdapter: ApiAuthAdapter | null = null
let refreshInFlight: { sessionId: number | null; promise: Promise<string> } | null = null

export function configureApiAuth(adapter: ApiAuthAdapter | null): void {
  authAdapter = adapter
  if (!adapter) refreshInFlight = null
}

export function apiUrl(path: string): string {
  const normalized = path.startsWith('/') ? path : `/${path}`
  return `${API_BASE_URL}${normalized}`
}

export async function requestJson<T>(
  path: string,
  init: RequestInit = {},
  options: RequestJsonOptions = {},
): Promise<T> {
  const authenticated = options.authenticated ?? true
  const retryOnUnauthorized = options.retryOnUnauthorized ?? true
  const adapter = authAdapter
  const initialAccessToken = authenticated ? adapter?.getAccessToken() ?? null : null
  const initialSessionId = authenticated ? adapter?.getSessionId?.() ?? null : null
  const firstResponse = await send(path, init, initialAccessToken)

  if (firstResponse.status === 401 && authenticated && retryOnUnauthorized && adapter) {
    const currentAccessToken = adapter.getAccessToken()
    const currentSessionId = adapter.getSessionId?.() ?? null
    if (adapter.getSessionId && currentSessionId !== initialSessionId) return readResponse<T>(firstResponse)

    const accessToken = currentAccessToken && currentAccessToken !== initialAccessToken
      ? currentAccessToken
      : await refreshOnce(adapter, currentSessionId)
    if (adapter.getSessionId && adapter.getSessionId() !== initialSessionId) return readResponse<T>(firstResponse)

    const retryResponse = await send(path, init, accessToken)
    if (
      retryResponse.status === 401
      && (!adapter.getSessionId || adapter.getSessionId() === initialSessionId)
    ) {
      adapter.onAuthorizationFailure?.()
    }
    return readResponse<T>(retryResponse)
  }

  return readResponse<T>(firstResponse)
}

async function refreshOnce(adapter: ApiAuthAdapter, sessionId: number | null): Promise<string> {
  if (!refreshInFlight || refreshInFlight.sessionId !== sessionId) {
    const refresh = adapter.refreshAccessToken()
    const promise = refresh.finally(() => {
      if (refreshInFlight?.promise === promise) refreshInFlight = null
    })
    refreshInFlight = { sessionId, promise }
  }
  return refreshInFlight.promise
}

async function send(path: string, init: RequestInit, accessToken: string | null): Promise<Response> {
  const headers = new Headers(init.headers)
  if (!headers.has('Accept')) headers.set('Accept', 'application/json')
  if (!headers.has('Content-Type')) headers.set('Content-Type', 'application/json')
  if (accessToken) headers.set('Authorization', `Bearer ${accessToken}`)
  else headers.delete('Authorization')

  try {
    return await fetch(apiUrl(path), { ...init, headers })
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') throw error
    throw new ApiNetworkError('The server could not be reached.', error)
  }
}

async function readResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as ApiErrorBody | null
    throw new ApiRequestError(
      response.status,
      body?.code ?? 'UNKNOWN_ERROR',
      body?.message ?? response.statusText,
      body?.details ?? [],
    )
  }

  if (response.status === 204) return undefined as T
  const text = await response.text()
  return (text ? JSON.parse(text) : undefined) as T
}

export class ApiRequestError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
    readonly details: string[] = [],
  ) {
    super(message)
    this.name = 'ApiRequestError'
  }
}

export class ApiNetworkError extends Error {
  constructor(message: string, readonly cause?: unknown) {
    super(message)
    this.name = 'ApiNetworkError'
  }
}
