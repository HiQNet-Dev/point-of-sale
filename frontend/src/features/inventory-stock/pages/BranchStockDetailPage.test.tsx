// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { AuthSessionContext } from '../../auth/model/authSessionContext'
import type { AuthSessionValue } from '../../auth/model/authSessionContext'
import type { AuthMe } from '../../auth/api/authApi'
import type { Branch } from '../../pos-session/api/posSessionApi'
import type { InventoryMovement, InventoryMovementType, BranchStock } from '../api/inventoryStockApi'
import type { PageResponse } from '../../../shared/api/pagination'
import { BranchStockDetailPage } from './BranchStockDetailPage'

const ORG = 'org-1'

const branch: Branch = {
  id: 'b-1',
  organizationId: ORG,
  name: 'Centro',
  code: 'CTR',
  timezone: 'America/Mexico_City',
  status: 'ACTIVE',
}

const stock: BranchStock = {
  inventoryItemId: 'item-1',
  code: 'harina-1kg',
  name: 'Harina 1kg',
  baseUnit: 'UNIT',
  fractionalUnitAllowed: false,
  status: 'ACTIVE',
  quantityOnHand: '972.500000',
  version: 3,
}

const movements: InventoryMovement[] = [
  {
    id: 'm1',
    type: 'OPENING',
    quantityDelta: '975.000000',
    resultingQuantity: '975.000000',
    reason: null,
    actorUserId: 'u1',
    saleId: null,
    occurredAt: '2026-10-01T12:00:00Z',
  },
  {
    id: 'm2',
    type: 'ADJUSTMENT',
    quantityDelta: '-2.500000',
    resultingQuantity: '972.500000',
    reason: 'Merma',
    actorUserId: 'u1',
    saleId: null,
    occurredAt: '2026-10-02T12:00:00Z',
  },
  {
    id: 'm3',
    type: 'SALE_CONSUMPTION',
    quantityDelta: '-1.000000',
    resultingQuantity: '971.500000',
    reason: null,
    actorUserId: 'u1',
    saleId: 'sale-9',
    occurredAt: '2026-10-03T12:00:00Z',
  },
  {
    id: 'm4',
    type: 'REVERSAL' as InventoryMovementType,
    quantityDelta: '1.000000',
    resultingQuantity: '972.500000',
    reason: null,
    actorUserId: 'u1',
    saleId: null,
    occurredAt: '2026-10-04T12:00:00Z',
  },
]

function authValue(): AuthSessionValue {
  const me: AuthMe = {
    userId: 'user-1',
    organizationId: ORG,
    email: 'admin@example.com',
    status: 'ACTIVE',
    organization: { currency: 'MXN', locale: 'es-MX', timezone: 'America/Mexico_City' },
  }
  return { status: 'AUTHENTICATED', me, logoutNotice: null, login: async () => me, logout: async () => true }
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

function page<T>(items: T[]): PageResponse<T> {
  return { items, page: 0, size: 20, totalElements: items.length, totalPages: items.length ? 1 : 0 }
}

function createBackend() {
  const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
    const url = new URL(String(input), 'http://localhost')
    if (url.pathname.endsWith('/auth/me/branches')) return json(page([branch]))
    if (url.pathname.includes('/movements')) return json(page(movements))
    if (url.pathname.endsWith('/inventory-items/item-1')) return json(stock)
    return new Response(null, { status: 204 })
  })
  return { fetchMock }
}

function renderPage(backend: ReturnType<typeof createBackend>) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  vi.stubGlobal('fetch', backend.fetchMock)
  render(
    <QueryClientProvider client={queryClient}>
      <AuthSessionContext.Provider value={authValue()}>
        <MemoryRouter initialEntries={['/admin/inventory/stock/item-1?branch=b-1']}>
          <Routes>
            <Route path="/admin/inventory/stock/:itemId" element={<BranchStockDetailPage />} />
          </Routes>
        </MemoryRouter>
      </AuthSessionContext.Provider>
    </QueryClientProvider>,
  )
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('BranchStockDetailPage', () => {
  it('shows the selected branch, item and exact quantity', async () => {
    const backend = createBackend()
    renderPage(backend)

    expect(await screen.findByText('Harina 1kg')).toBeTruthy()
    expect(screen.getByText('harina-1kg')).toBeTruthy()
    expect(await screen.findByText(/Centro · CTR/)).toBeTruthy()
  })

  it('labels movement types and falls back gracefully for unknown ones', async () => {
    const backend = createBackend()
    renderPage(backend)

    expect(await screen.findByText('Apertura')).toBeTruthy()
    expect(screen.getByText('Ajuste')).toBeTruthy()
    expect(screen.getByText('Venta')).toBeTruthy()
    expect(screen.getByText('REVERSAL')).toBeTruthy()
  })

  it('renders a dash for null movement reasons instead of null/undefined', async () => {
    const backend = createBackend()
    renderPage(backend)

    await screen.findByText('Apertura')
    expect(screen.getByText('Merma')).toBeTruthy()
    expect(screen.getAllByText('—').length).toBeGreaterThanOrEqual(1)
    expect(screen.queryByText('null')).toBeNull()
  })

  it('disables opening once the item already has movements', async () => {
    const backend = createBackend()
    renderPage(backend)

    const opening = await screen.findByRole('button', { name: 'Apertura de existencias' })
    expect((opening as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getByText('La apertura solo se permite antes de cualquier movimiento de existencias.')).toBeTruthy()
  })
})
