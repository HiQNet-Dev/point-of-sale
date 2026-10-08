import { describe, expect, it, vi } from 'vitest'
import { apiUrl, requestJson } from './client'

describe('apiUrl', () => {
  it('joins a path to the configured base URL', () => {
    expect(apiUrl('health')).toMatch(/\/api\/v1\/health$/)
  })

  it('handles paths starting with a slash', () => {
    expect(apiUrl('/catalog/products')).toMatch(/\/api\/v1\/catalog\/products$/)
  })
})

describe('requestJson', () => {
  it('throws ApiRequestError with the backend error code', async () => {
    const body = { code: 'RESOURCE_NOT_FOUND', message: 'Resource not found' }
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: false,
        status: 404,
        json: async () => body,
      }),
    )

    await expect(requestJson('/missing')).rejects.toMatchObject({
      name: 'ApiRequestError',
      status: 404,
      code: 'RESOURCE_NOT_FOUND',
    })

    vi.unstubAllGlobals()
  })
})