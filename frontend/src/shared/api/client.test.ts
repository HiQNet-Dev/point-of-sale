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
      '{"sale":{"totalAmount":999999999999999.9999,"nullableAmount":null},"items":[{"lineTotal":0.0001,"quantity":2,"modifiers":[{"priceDeltaSnapshot":-0.5000,"exponentAmount":1e-4}]}],"note":' + JSON.stringify('literal "totalAmount":5') + ',"version":7}',
      { headers: { 'Content-Type': 'application/json' } },
    )))

    const response = await requestJson<{
      sale: { totalAmount: string; nullableAmount: null }
      items: Array<{
        lineTotal: string
        quantity: number
        modifiers: Array<{ priceDeltaSnapshot: string; exponentAmount: string }>
      }>
      note: string
      version: number
    }>('/exact', {}, { exactDecimalFields: ['totalAmount', 'lineTotal', 'priceDeltaSnapshot', 'exponentAmount'] })

    expect(response.sale.totalAmount).toBe('999999999999999.9999')
    expect(response.items[0].lineTotal).toBe('0.0001')
    expect(response.items[0].quantity).toBe(2)
    expect(response.items[0].modifiers[0].priceDeltaSnapshot).toBe('-0.5000')
    expect(response.items[0].modifiers[0].exponentAmount).toBe('1e-4')
    expect(response.sale.nullableAmount).toBeNull()
    expect(response.version).toBe(7)
    expect(response.note).toBe('literal "totalAmount":5')
    vi.unstubAllGlobals()
  })

  it('keeps default endpoint numbers as JavaScript numbers', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('{"version":7,"amount":123.4567}', {
      headers: { 'Content-Type': 'application/json' },
    })))
    const response = await requestJson<{ version: number; amount: number }>('/normal')
    expect(response.version).toBe(7)
    expect(typeof response.version).toBe('number')
    expect(typeof response.amount).toBe('number')
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