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
  exactDecimalFields?: readonly string[]
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
    if (adapter.getSessionId && currentSessionId !== initialSessionId) {
      return readResponse<T>(firstResponse, options.exactDecimalFields)
    }

    const accessToken = currentAccessToken && currentAccessToken !== initialAccessToken
      ? currentAccessToken
      : await refreshOnce(adapter, currentSessionId)
    if (adapter.getSessionId && adapter.getSessionId() !== initialSessionId) {
      return readResponse<T>(firstResponse, options.exactDecimalFields)
    }

    const retryResponse = await send(path, init, accessToken)
    if (
      retryResponse.status === 401
      && (!adapter.getSessionId || adapter.getSessionId() === initialSessionId)
    ) {
      adapter.onAuthorizationFailure?.()
    }
    return readResponse<T>(retryResponse, options.exactDecimalFields)
  }

  return readResponse<T>(firstResponse, options.exactDecimalFields)
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

async function readResponse<T>(response: Response, exactDecimalFields?: readonly string[]): Promise<T> {
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
  if (!text) return undefined as T
  return (exactDecimalFields?.length
    ? parseJsonWithExactDecimals(text, new Set(exactDecimalFields))
    : JSON.parse(text)) as T
}

function parseJsonWithExactDecimals(text: string, exactFields: ReadonlySet<string>): unknown {
  let index = 0

  function skipWhitespace() {
    while (/\s/.test(text[index] ?? '') && index < text.length) index += 1
  }

  function readString(): string {
    const start = index
    index += 1
    while (index < text.length) {
      if (text[index] === '\\') {
        index += 2
      } else if (text[index] === '"') {
        index += 1
        return JSON.parse(text.slice(start, index)) as string
      } else {
        index += 1
      }
    }
    throw new SyntaxError('Unterminated JSON string')
  }

  function readValue(field?: string): unknown {
    skipWhitespace()
    const current = text[index]
    if (current === '"') return readString()
    if (current === '{') {
      index += 1
      skipWhitespace()
      const object: Record<string, unknown> = {}
      if (text[index] === '}') {
        index += 1
        return object
      }
      while (index < text.length) {
        skipWhitespace()
        if (text[index] !== '"') throw new SyntaxError('Expected JSON object key')
        const key = readString()
        skipWhitespace()
        if (text[index] !== ':') throw new SyntaxError('Expected colon after JSON object key')
        index += 1
        Object.defineProperty(object, key, {
          value: readValue(key), enumerable: true, configurable: true, writable: true,
        })
        skipWhitespace()
        if (text[index] === '}') {
          index += 1
          return object
        }
        if (text[index] !== ',') throw new SyntaxError('Expected comma in JSON object')
        index += 1
      }
      throw new SyntaxError('Unterminated JSON object')
    }
    if (current === '[') {
      index += 1
      skipWhitespace()
      const array: unknown[] = []
      if (text[index] === ']') {
        index += 1
        return array
      }
      while (index < text.length) {
        array.push(readValue())
        skipWhitespace()
        if (text[index] === ']') {
          index += 1
          return array
        }
        if (text[index] !== ',') throw new SyntaxError('Expected comma in JSON array')
        index += 1
      }
      throw new SyntaxError('Unterminated JSON array')
    }

    const start = index
    while (index < text.length && !/[\s,\]}]/.test(text[index])) index += 1
    const token = text.slice(start, index)
    if (field && exactFields.has(field)
      && /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$/.test(token)) return token
    return JSON.parse(token) as unknown
  }

  const value = readValue()
  skipWhitespace()
  if (index !== text.length) throw new SyntaxError('Unexpected content after JSON value')
  return value
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
