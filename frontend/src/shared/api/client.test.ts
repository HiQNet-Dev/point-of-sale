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
  it('preserves selected JSON decimal tokens without changing non-money fields or quoted text', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(
      '{"sale":{"totalAmount":999999999999999.9999},"items":[{"lineTotal":0.0001,"quantity":2}],"note":' + JSON.stringify('literal "totalAmount":5') + ',"version":7}',
      { headers: { 'Content-Type': 'application/json' } },
    )))

    const response = await requestJson<{
      sale: { totalAmount: string }
      items: Array<{ lineTotal: string; quantity: number }>
      note: string
      version: number
    }>('/exact', {}, { exactDecimalFields: ['totalAmount', 'lineTotal'] })

    expect(response.sale.totalAmount).toBe('999999999999999.9999')
    expect(response.items[0].lineTotal).toBe('0.0001')
    expect(response.items[0].quantity).toBe(2)
    expect(response.version).toBe(7)
    expect(response.note).toBe('literal "totalAmount":5')
    vi.unstubAllGlobals()
  })

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