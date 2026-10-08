import { API_BASE_URL } from '../../app/config/env'

/**
 * Shared API client foundation.
 * Business modules build their typed calls on top of these helpers;
 * pages never call fetch directly.
 */

export function apiUrl(path: string): string {
  const normalized = path.startsWith('/') ? path : `/${path}`
  return `${API_BASE_URL}${normalized}`
}

/** Public error contract returned by the backend (see backend ApiError). */
export interface ApiErrorBody {
  timestamp: string
  status: number
  code: string
  message: string
  path: string
  details: string[]
}

export async function requestJson<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(apiUrl(path), {
    ...init,
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/json',
      ...init.headers,
    },
  })

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as ApiErrorBody | null
    throw new ApiRequestError(response.status, body?.code ?? 'UNKNOWN_ERROR', body?.message ?? response.statusText)
  }

  const text = await response.text()
  return (text ? JSON.parse(text) : undefined) as T
}

export class ApiRequestError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message)
    this.name = 'ApiRequestError'
  }
}