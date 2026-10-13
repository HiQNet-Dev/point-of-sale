import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  getBranchStock,
  listBranchStock,
  listInventoryMovements,
  recordOpeningStock,
  recordStockAdjustment,
} from './inventoryStockApi'

interface Call {
  url: URL
  method: string
  body: unknown
}

function capture(): { calls: Call[]; fetchMock: ReturnType<typeof vi.fn> } {
  const calls: Call[] = []
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push({
      url: new URL(String(input), 'http://localhost'),
      method: init?.method ?? 'GET',
      body: init?.body ? JSON.parse(String(init.body)) : null,
    })
    return new Response(JSON.stringify({ items: [], page: 0, size: 20, totalElements: 0, totalPages: 0 }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    })
  })
  vi.stubGlobal('fetch', fetchMock)
  return { calls, fetchMock }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('inventory stock API contract', () => {
  it('scopes list reads to the branch and forwards filters', async () => {
    const { calls } = capture()
    await listBranchStock('org-1', 'b-1', { page: 1, size: 20, q: 'harina', status: 'ACTIVE', baseUnit: 'GRAM' })

    expect(calls[0].method).toBe('GET')
    expect(calls[0].url.pathname).toBe('/api/v1/organizations/org-1/branches/b-1/inventory-items')
    expect(calls[0].url.searchParams.get('page')).toBe('1')
    expect(calls[0].url.searchParams.get('q')).toBe('harina')
    expect(calls[0].url.searchParams.get('status')).toBe('ACTIVE')
    expect(calls[0].url.searchParams.get('baseUnit')).toBe('GRAM')
  })

  it('reads an item detail and its movement history with paging', async () => {
    const { calls } = capture()
    await getBranchStock('org-1', 'b-1', 'item-1')
    await listInventoryMovements('org-1', 'b-1', 'item-1', 2, 20)

    expect(calls[0].url.pathname).toBe('/api/v1/organizations/org-1/branches/b-1/inventory-items/item-1')
    expect(calls[1].url.pathname).toBe('/api/v1/organizations/org-1/branches/b-1/inventory-items/item-1/movements')
    expect(calls[1].url.searchParams.get('page')).toBe('2')
    expect(calls[1].url.searchParams.get('size')).toBe('20')
  })

  it('sends opening and adjustment quantities as exact decimal strings', async () => {
    const { calls } = capture()
    await recordOpeningStock('org-1', 'b-1', 'item-1', { expectedVersion: 0, quantity: '10.5' })
    await recordStockAdjustment('org-1', 'b-1', 'item-1', {
      expectedVersion: 0,
      quantityDelta: '-2.5',
      reason: 'Merma',
    })

    expect(calls[0].method).toBe('POST')
    expect(calls[0].url.pathname).toBe('/api/v1/organizations/org-1/branches/b-1/inventory-items/item-1/opening-stock')
    expect(calls[0].body).toEqual({ expectedVersion: 0, quantity: '10.5' })

    expect(calls[1].url.pathname).toBe('/api/v1/organizations/org-1/branches/b-1/inventory-items/item-1/adjustments')
    expect(calls[1].body).toEqual({ expectedVersion: 0, quantityDelta: '-2.5', reason: 'Merma' })
  })
})
