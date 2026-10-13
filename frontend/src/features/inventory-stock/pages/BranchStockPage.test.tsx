// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { AuthSessionContext } from '../../auth/model/authSessionContext'
import type { AuthSessionValue } from '../../auth/model/authSessionContext'
import type { AuthMe } from '../../auth/api/authApi'
import type { Branch } from '../../pos-session/api/posSessionApi'
import type { BranchStock } from '../api/inventoryStockApi'
import type { PageResponse } from '../../../shared/api/pagination'
import { BranchStockPage } from './BranchStockPage'

const ORG = 'org-1'

const branchA: Branch = {
  id: 'branch-a',
  organizationId: ORG,
  name: 'Centro',
  code: 'CTR',
  timezone: 'America/Mexico_City',
  status: 'ACTIVE',
}
const branchB: Branch = { ...branchA, id: 'branch-b', name: 'Norte', code: 'NTE' }

function stock(overrides: Partial<BranchStock>): BranchStock {
  return {
    inventoryItemId: 'item-a',
    code: 'harina-1kg',
    name: 'Harina 1kg',
    baseUnit: 'UNIT',
    fractionalUnitAllowed: false,
    status: 'ACTIVE',
    quantityOnHand: '0.000000',
    version: 0,
    ...overrides,
  }
}

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

interface BackendOptions {
  branches?: Branch[]
  stockA?: BranchStock[]
  stockB?: BranchStock[]
  stockStatus?: number
}

function createBackend(options: BackendOptions = {}) {
  const branches = options.branches ?? [branchA, branchB]
  const stockRequests: URL[] = []
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input), 'http://localhost')
    if (url.pathname.endsWith('/auth/me/branches')) return json(page(branches))
    if (url.pathname.endsWith('/inventory-items') && (init?.method ?? 'GET') === 'GET') {
      stockRequests.push(url)
      if (options.stockStatus) return json({ code: 'FORBIDDEN', message: 'Forbidden' }, options.stockStatus)
      const branchId = url.pathname.split('/branches/')[1]?.split('/')[0]
      return json(page(branchId === branchB.id ? options.stockB ?? [] : options.stockA ?? []))
    }
    return new Response(null, { status: 204 })
  })
  return { fetchMock, stockRequests }
}

function renderPage(backend: ReturnType<typeof createBackend>) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  vi.stubGlobal('fetch', backend.fetchMock)
  render(
    <QueryClientProvider client={queryClient}>
      <AuthSessionContext.Provider value={authValue()}>
        <MemoryRouter initialEntries={['/admin/inventory/stock']}>
          <Routes>
            <Route path="/admin/inventory/stock" element={<BranchStockPage />} />
          </Routes>
        </MemoryRouter>
      </AuthSessionContext.Provider>
    </QueryClientProvider>,
  )
}

async function chooseBranch(option: RegExp) {
  fireEvent.mouseDown(await screen.findByRole('combobox', { name: 'Sucursal' }))
  fireEvent.click(await screen.findByRole('option', { name: option }))
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('BranchStockPage', () => {
  it('requires an explicit branch before showing any stock', async () => {
    const backend = createBackend({ stockA: [stock({ quantityOnHand: '975.000000' })] })
    renderPage(backend)

    expect(await screen.findByText('Selecciona una sucursal para ver sus existencias.')).toBeTruthy()
    expect(backend.stockRequests).toHaveLength(0)
  })

  it('lists branch stock with quantities rendered from exact decimal strings', async () => {
    const backend = createBackend({ stockA: [stock({ quantityOnHand: '975.000000' })] })
    renderPage(backend)

    await chooseBranch(/Centro · CTR/)

    expect(await screen.findByText('Harina 1kg')).toBeTruthy()
    expect(screen.getByText('975')).toBeTruthy()
    expect(backend.stockRequests[0].pathname).toContain(`/organizations/${ORG}/branches/${branchA.id}/inventory-items`)
    const link = screen.getByRole('link', { name: 'Ver' })
    expect(link.getAttribute('href')).toBe('/admin/inventory/stock/item-a?branch=branch-a')
  })

  it('refetches scoped stock when the branch changes', async () => {
    const backend = createBackend({
      stockA: [stock({ quantityOnHand: '975.000000' })],
      stockB: [stock({ inventoryItemId: 'item-b', code: 'azucar', name: 'Azúcar', quantityOnHand: '125.500000' })],
    })
    renderPage(backend)

    await chooseBranch(/Centro · CTR/)
    expect(await screen.findByText('Harina 1kg')).toBeTruthy()

    await chooseBranch(/Norte · NTE/)
    expect(await screen.findByText('Azúcar')).toBeTruthy()
    expect(screen.getByText('125.5')).toBeTruthy()
    await waitFor(() => expect(screen.queryByText('Harina 1kg')).toBeNull())
    expect(backend.stockRequests.some((url) => url.pathname.includes(`/branches/${branchB.id}/`))).toBe(true)
  })

  it('shows an empty self-scoped branch result', async () => {
    const backend = createBackend({ branches: [] })
    renderPage(backend)

    expect(await screen.findByText('No tienes sucursales asignadas.')).toBeTruthy()
  })

  it('surfaces a 403 on the stock read with a friendly message', async () => {
    const backend = createBackend({ stockStatus: 403 })
    renderPage(backend)

    await chooseBranch(/Centro · CTR/)

    expect(await screen.findByText('No tienes permiso para esta operación de existencias.')).toBeTruthy()
  })
})
