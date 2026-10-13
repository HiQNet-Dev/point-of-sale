/**
 * Server pagination envelope shared by list endpoints
 * (items/page/size/totalElements/totalPages). Mirrors the backend contract.
 */
export interface PageResponse<T> {
  items: T[]
  page: number
  size: number
  totalElements: number
  totalPages: number
}

export interface ListQuery {
  page: number
  size: number
  q?: string
}

/** Builds the paging query string with an optional search term and extra filters. */
export function pagingParams(query: ListQuery, extra?: Record<string, string | undefined>): string {
  const params = new URLSearchParams({ page: String(query.page), size: String(query.size) })
  if (query.q?.trim()) params.set('q', query.q.trim())
  if (extra) {
    for (const [key, value] of Object.entries(extra)) {
      if (value) params.set(key, value)
    }
  }
  return params.toString()
}

/** Encodes a value for safe use inside a URL path segment. */
export function segment(value: string): string {
  return encodeURIComponent(value)
}
