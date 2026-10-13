// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { AuthSessionContext } from '../../auth/model/authSessionContext'
import type { AuthSessionValue } from '../../auth/model/authSessionContext'
import type { AuthMe } from '../../auth/api/authApi'
import type { PageResponse } from '../../../shared/api/pagination'
import { RecipesPage } from './RecipesPage'

const ORG = 'org-1'

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

const product = { id: 'p-1', organizationId: ORG, categoryId: null, sku: 'HAR-1', name: 'Harina 1kg', status: 'ACTIVE', version: 1 }
const item = {
  id: 'item-1', organizationId: ORG, code: 'HAR-1', name: 'Harina 1kg',
  description: null, baseUnit: 'GRAM', fractionalUnitAllowed: true, status: 'ACTIVE', version: 1,
  createdAt: '2026-01-01T00:00:00Z', updatedAt: '2026-01-01T00:00:00Z',
}

interface BackendOptions {
  recipe?: Record<string, unknown> | null
  recipeStatus?: number
  recipeCode?: string
  putResponse?: Record<string, unknown>
  putStatus?: number
  putCode?: string
  itemStatus?: 'ACTIVE' | 'INACTIVE'
}

function createBackend(options: BackendOptions = {}) {
  const puts: Array<Record<string, unknown>> = []
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input), 'http://localhost')
    const method = init?.method ?? 'GET'
    if (url.pathname.endsWith('/products/p-1/inventory-recipe')) {
      if (method === 'PUT') {
        puts.push(JSON.parse(String(init?.body)))
        if (options.putStatus && options.putStatus >= 400) {
          return json({ code: options.putCode ?? 'ERROR', message: 'x' }, options.putStatus)
        }
        return json(options.putResponse ?? { id: 'r-1', organizationId: ORG, productId: 'p-1', status: 'ACTIVE', version: 2, createdAt: 'x', updatedAt: 'x', components: [] })
      }
      if (options.recipeStatus && options.recipeStatus >= 400) {
        return json({ code: options.recipeCode ?? 'INVENTORY_RECIPE_NOT_FOUND', message: 'x' }, options.recipeStatus)
      }
      return json(options.recipe ?? null)
    }
    if (url.pathname.endsWith('/products')) return json(page([product]))
    if (url.pathname.endsWith('/categories')) return json(page([]))
    if (url.pathname.endsWith('/inventory-items')) {
      return json(page([{ ...item, status: options.itemStatus ?? 'ACTIVE' }]))
    }
    return new Response(null, { status: 204 })
  })
  return { fetchMock, puts }
}

function renderPage(backend: ReturnType<typeof createBackend>) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  vi.stubGlobal('fetch', backend.fetchMock)
  render(
    <QueryClientProvider client={queryClient}>
      <AuthSessionContext.Provider value={authValue()}>
        <MemoryRouter initialEntries={['/admin/inventory/recipes']}>
          <RecipesPage />
        </MemoryRouter>
      </AuthSessionContext.Provider>
    </QueryClientProvider>,
  )
}

async function selectProduct() {
  expect(await screen.findByText('Harina 1kg')).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Configurar receta' }))
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('RecipesPage', () => {
  it('lists products and shows the absent-recipe empty state', async () => {
    const backend = createBackend({ recipeStatus: 404, recipeCode: 'INVENTORY_RECIPE_NOT_FOUND' })
    renderPage(backend)
    await selectProduct()

    expect(await screen.findByText('Este producto aún no tiene una receta de inventario.')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Guardar receta' })).toBeTruthy()
  })

  it('loads an existing recipe and saves exact decimal strings with the response version', async () => {
    const backend = createBackend({
      recipe: {
        id: 'r-1', organizationId: ORG, productId: 'p-1', status: 'ACTIVE', version: 3,
        createdAt: 'x', updatedAt: 'x',
        components: [{ inventoryItemId: 'item-1', code: 'HAR-1', name: 'Harina 1kg', baseUnit: 'GRAM', fractionalUnitAllowed: true, status: 'ACTIVE', quantity: '18.500000' }],
      },
    })
    renderPage(backend)
    await selectProduct()

    const quantity = await screen.findByLabelText('Cantidad')
    expect((quantity as HTMLInputElement).value).toBe('18.5')
    fireEvent.change(quantity, { target: { value: '20' } })
    fireEvent.click(screen.getByRole('button', { name: 'Guardar receta' }))

    await waitFor(() => expect(backend.puts.length).toBe(1))
    expect(backend.puts[0]).toEqual({
      expectedVersion: 3,
      status: 'ACTIVE',
      components: [{ inventoryItemId: 'item-1', quantity: '20' }],
    })
  })

  it('blocks saving an ACTIVE recipe with no components', async () => {
    const backend = createBackend({ recipeStatus: 404, recipeCode: 'INVENTORY_RECIPE_NOT_FOUND' })
    renderPage(backend)
    await selectProduct()

    await screen.findByText('Este producto aún no tiene una receta de inventario.')
    fireEvent.click(screen.getByRole('button', { name: 'Guardar receta' }))

    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toContain('Una receta activa debe tener al menos un componente.')
    expect(backend.puts.length).toBe(0)
  })

  it('rejects fractional quantities for a discrete UNIT item', async () => {
    const backend = createBackend({
      recipe: {
        id: 'r-1', organizationId: ORG, productId: 'p-1', status: 'ACTIVE', version: 1,
        createdAt: 'x', updatedAt: 'x',
        components: [{ inventoryItemId: 'item-1', code: 'CAF-1', name: 'Café', baseUnit: 'UNIT', fractionalUnitAllowed: false, status: 'ACTIVE', quantity: '1.000000' }],
      },
    })
    renderPage(backend)
    await selectProduct()

    const quantity = await screen.findByLabelText('Cantidad')
    fireEvent.change(quantity, { target: { value: '0.5' } })
    fireEvent.click(screen.getByRole('button', { name: 'Guardar receta' }))

    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toContain('Este artículo no admite fracciones; usa una cantidad entera.')
    expect(backend.puts.length).toBe(0)
  })

  it('keeps an existing inactive component visible', async () => {
    const backend = createBackend({
      recipe: {
        id: 'r-1', organizationId: ORG, productId: 'p-1', status: 'INACTIVE', version: 2,
        createdAt: 'x', updatedAt: 'x',
        components: [{ inventoryItemId: 'item-1', code: 'CAF-1', name: 'Café en grano', baseUnit: 'GRAM', fractionalUnitAllowed: true, status: 'INACTIVE', quantity: '1000.000000' }],
      },
    })
    renderPage(backend)
    await selectProduct()

    const picker = (await screen.findByLabelText('Artículo')) as HTMLInputElement
    await waitFor(() => expect(picker.value).toBe('Café en grano · CAF-1'))
    expect(screen.getByText('Inactiva')).toBeTruthy()
  })

  it('handles a concurrent modification on save', async () => {
    const backend = createBackend({
      recipeStatus: 404, recipeCode: 'INVENTORY_RECIPE_NOT_FOUND',
      putStatus: 409, putCode: 'CONCURRENT_MODIFICATION',
    })
    renderPage(backend)
    await selectProduct()

    await screen.findByText('Este producto aún no tiene una receta de inventario.')
    // Add a component through the selector so the form is valid.
    fireEvent.click(screen.getByRole('button', { name: 'Añadir componente' }))
    const picker = screen.getByLabelText('Artículo')
    fireEvent.focus(picker)
    fireEvent.click(await screen.findByRole('option', { name: /Harina 1kg/ }))
    fireEvent.change(screen.getByLabelText('Cantidad'), { target: { value: '5' } })
    fireEvent.click(screen.getByRole('button', { name: 'Guardar receta' }))

    const banner = await screen.findByRole('status')
    expect(banner.textContent).toContain('La receta cambió mientras la editabas')
  })

  it('shows a controlled message when the recipe read is forbidden', async () => {
    const backend = createBackend({ recipeStatus: 403, recipeCode: 'FORBIDDEN' })
    renderPage(backend)
    await selectProduct()

    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toContain('No tienes permiso para administrar recetas de inventario.')
  })
})
