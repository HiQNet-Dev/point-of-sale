// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { AuthSessionContext } from '../../auth/model/authSessionContext'
import type { AuthSessionValue } from '../../auth/model/authSessionContext'
import type { AuthMe } from '../../auth/api/authApi'
import type { InventoryItem } from '../api/inventoryItemsApi'
import type { PageResponse } from '../../../shared/api/pagination'
import { InventoryItemsPage } from './InventoryItemsPage'

const ORG = 'org-1'

const itemA: InventoryItem = {
  id: 'item-a',
  organizationId: ORG,
  code: 'harina-1kg',
  name: 'Harina 1kg',
  description: 'Harina de trigo',
  baseUnit: 'UNIT',
  fractionalUnitAllowed: false,
  status: 'ACTIVE',
  version: 3,
  createdAt: '2026-10-01T00:00:00Z',
  updatedAt: '2026-10-01T00:00:00Z',
}

function authValue(): AuthSessionValue {
  const me: AuthMe = {
    userId: 'user-1',
    organizationId: ORG,
    email: 'admin@example.com',
    status: 'ACTIVE',
    organization: { currency: 'MXN', locale: 'es-MX', timezone: 'America/Mexico_City' },
  }
  return {
    status: 'AUTHENTICATED',
    me,
    logoutNotice: null,
    login: async () => me,
    logout: async () => true,
  }
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

function page(items: InventoryItem[], totalElements = items.length, totalPages = 1): PageResponse<InventoryItem> {
  return { items, page: 0, size: 20, totalElements, totalPages }
}

interface BackendOptions {
  items?: InventoryItem[]
  totalPages?: number
  createStatus?: number
  createCode?: string
  patchStatus?: number
  patchCode?: string
}

function createBackend(options: BackendOptions = {}) {
  let items = options.items ?? [itemA]
  const listRequests: URL[] = []
  const createBodies: unknown[] = []
  const patchBodies: unknown[] = []

  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input), 'http://localhost')
    const method = init?.method ?? 'GET'
    if (url.pathname.endsWith('/inventory-items') && method === 'GET') {
      listRequests.push(url)
      return json(page(items, items.length, options.totalPages ?? 1))
    }
    if (url.pathname.endsWith('/inventory-items') && method === 'POST') {
      const body = JSON.parse(String(init?.body)) as Partial<InventoryItem> & { code: string; name: string }
      createBodies.push(body)
      if (options.createStatus) return json({ code: options.createCode ?? 'ERROR', message: 'x' }, options.createStatus)
      const created: InventoryItem = {
        ...itemA,
        id: 'created-1',
        code: body.code,
        name: body.name,
        description: body.description ?? null,
        version: 0,
      }
      items = [created, ...items]
      return json(created, 201)
    }
    if (/\/inventory-items\/[^/]+$/.test(url.pathname) && method === 'PATCH') {
      const body = JSON.parse(String(init?.body)) as Partial<InventoryItem>
      patchBodies.push(body)
      if (options.patchStatus) return json({ code: options.patchCode ?? 'ERROR', message: 'x' }, options.patchStatus)
      items = items.map((item) => (item.id === 'item-a' ? { ...item, ...body } : item))
      return json(items.find((item) => item.id === 'item-a'))
    }
    return new Response(null, { status: 204 })
  })

  return { fetchMock, listRequests, createBodies, patchBodies }
}

function renderPage(backend: ReturnType<typeof createBackend>) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  vi.stubGlobal('fetch', backend.fetchMock)
  render(
    <QueryClientProvider client={queryClient}>
      <AuthSessionContext.Provider value={authValue()}>
        <MemoryRouter initialEntries={['/admin/inventory/items']}>
          <Routes>
            <Route path="/admin/inventory/items" element={<InventoryItemsPage />} />
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

describe('InventoryItemsPage', () => {
  it('lists organization inventory items with base unit and status', async () => {
    const backend = createBackend()
    renderPage(backend)

    expect(await screen.findByText('Harina 1kg')).toBeTruthy()
    expect(screen.getByText('harina-1kg')).toBeTruthy()
    expect(screen.getByText('Activo')).toBeTruthy()
    expect(backend.listRequests[0].pathname).toContain(`/organizations/${ORG}/inventory-items`)
  })

  it('shows an empty state when there are no items', async () => {
    const backend = createBackend({ items: [] })
    renderPage(backend)

    expect(await screen.findByText('No hay artículos de inventario.')).toBeTruthy()
  })

  it('creates an item using a trimmed lowercase code without sending an expected version', async () => {
    const backend = createBackend()
    renderPage(backend)
    await screen.findByText('Harina 1kg')

    fireEvent.click(screen.getByRole('button', { name: 'Nuevo artículo' }))
    const dialog = screen.getByRole('dialog')
    fireEvent.change(within(dialog).getByLabelText(/Código/), { target: { value: ' cafe-molido ' } })
    fireEvent.change(within(dialog).getByLabelText(/Nombre/), { target: { value: 'Café molido' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Crear artículo' }))

    await waitFor(() => expect(backend.createBodies).toHaveLength(1))
    expect(backend.createBodies[0]).toEqual({
      code: 'cafe-molido',
      name: 'Café molido',
      baseUnit: 'UNIT',
      fractionalUnitAllowed: false,
    })
    expect(await screen.findByText('Café molido')).toBeTruthy()
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('rejects an invalid code slug before calling the backend', async () => {
    const backend = createBackend()
    renderPage(backend)
    await screen.findByText('Harina 1kg')

    fireEvent.click(screen.getByRole('button', { name: 'Nuevo artículo' }))
    const dialog = screen.getByRole('dialog')
    fireEvent.change(within(dialog).getByLabelText(/Código/), { target: { value: 'Codigo Invalido' } })
    fireEvent.change(within(dialog).getByLabelText(/Nombre/), { target: { value: 'X' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Crear artículo' }))

    expect(await screen.findByText('El código debe ser un slug en minúsculas (ej. cafe-molido).')).toBeTruthy()
    expect(backend.createBodies).toHaveLength(0)
  })

  it('edits an item with the expected version and its mutable fields', async () => {
    const backend = createBackend()
    renderPage(backend)
    await screen.findByText('Harina 1kg')

    fireEvent.click(screen.getAllByRole('button', { name: 'Editar' })[0])
    const dialog = screen.getByRole('dialog')
    fireEvent.change(within(dialog).getByLabelText(/Nombre/), { target: { value: 'Harina premium' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Guardar cambios' }))

    await waitFor(() => expect(backend.patchBodies).toHaveLength(1))
    expect(backend.patchBodies[0]).toEqual({
      expectedVersion: 3,
      name: 'Harina premium',
      description: 'Harina de trigo',
      baseUnit: 'UNIT',
      fractionalUnitAllowed: false,
    })
  })

  it('deactivates an active item with its current version', async () => {
    const backend = createBackend()
    renderPage(backend)
    await screen.findByText('Harina 1kg')

    fireEvent.click(screen.getByRole('button', { name: 'Desactivar' }))

    await waitFor(() => expect(backend.patchBodies).toHaveLength(1))
    expect(backend.patchBodies[0]).toEqual({ expectedVersion: 3, status: 'INACTIVE' })
  })

  it('reports a duplicate code conflict without closing the dialog', async () => {
    const backend = createBackend({ createStatus: 409, createCode: 'INVENTORY_ITEM_CODE_ALREADY_EXISTS' })
    renderPage(backend)
    await screen.findByText('Harina 1kg')

    fireEvent.click(screen.getByRole('button', { name: 'Nuevo artículo' }))
    const dialog = screen.getByRole('dialog')
    fireEvent.change(within(dialog).getByLabelText(/Código/), { target: { value: 'harina-1kg' } })
    fireEvent.change(within(dialog).getByLabelText(/Nombre/), { target: { value: 'Duplicado' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Crear artículo' }))

    expect(await screen.findByText('Ya existe un artículo con ese código en la organización.')).toBeTruthy()
    expect(screen.getByRole('dialog')).toBeTruthy()
  })
})
