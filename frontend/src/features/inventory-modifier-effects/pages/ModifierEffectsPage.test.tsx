// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { AuthSessionContext } from '../../auth/model/authSessionContext'
import type { AuthSessionValue } from '../../auth/model/authSessionContext'
import type { AuthMe } from '../../auth/api/authApi'
import type { PageResponse } from '../../../shared/api/pagination'
import { ModifierEffectsPage } from './ModifierEffectsPage'

const ORG = 'org-1'

const group = { id: 'g-1', organizationId: ORG, code: 'LECHE', name: 'Leche', status: 'ACTIVE' as const, version: 1 }
const option = { id: 'o-1', organizationId: ORG, modifierGroupId: 'g-1', code: 'LECH-N', name: 'Leche normal', displayOrder: 0, status: 'ACTIVE' as const, version: 1 }
const item = {
  id: 'item-active', organizationId: ORG, code: 'HAR-1', name: 'Harina 1kg', description: null,
  baseUnit: 'GRAM' as const, fractionalUnitAllowed: true, status: 'ACTIVE' as const, version: 1,
  createdAt: '2026-01-01T00:00:00Z', updatedAt: '2026-01-01T00:00:00Z',
}

function authValue(): AuthSessionValue {
  const me: AuthMe = {
    userId: 'user-1', organizationId: ORG, email: 'admin@example.com', status: 'ACTIVE',
    organization: { currency: 'MXN', locale: 'es-MX', timezone: 'America/Mexico_City' },
  }
  return { status: 'AUTHENTICATED', me, logoutNotice: null, login: async () => me, logout: async () => true }
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

function page<T>(items: T[]): PageResponse<T> {
  return { items, page: 0, size: 100, totalElements: items.length, totalPages: items.length ? 1 : 0 }
}

interface BackendOptions {
  config?: Record<string, unknown> | null
  configStatus?: number
  configCode?: string
  putResponse?: Record<string, unknown>
  putStatus?: number
  putCode?: string
}

function createBackend(options: BackendOptions = {}) {
  const puts: Array<Record<string, unknown>> = []
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input), 'http://localhost')
    const method = init?.method ?? 'GET'
    if (url.pathname.endsWith('/modifier-options/o-1/inventory-effects')) {
      if (method === 'PUT') {
        puts.push(JSON.parse(String(init?.body)))
        if (options.putStatus && options.putStatus >= 400) {
          return json({ code: options.putCode ?? 'ERROR', message: 'x' }, options.putStatus)
        }
        return json(options.putResponse ?? { id: 'e-1', organizationId: ORG, modifierOptionId: 'o-1', version: 2, createdAt: 'x', updatedAt: 'x', effects: [] })
      }
      if (options.configStatus && options.configStatus >= 400) {
        return json({ code: options.configCode ?? 'INVENTORY_MODIFIER_CONFIG_NOT_FOUND', message: 'x' }, options.configStatus)
      }
      return json(options.config ?? null)
    }
    if (url.pathname.endsWith('/options')) return json(page([option]))
    if (url.pathname.endsWith('/modifier-groups')) return json(page([group]))
    if (url.pathname.endsWith('/inventory-items')) return json(page([item]))
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
        <MemoryRouter initialEntries={['/admin/inventory/modifier-effects']}>
          <ModifierEffectsPage />
        </MemoryRouter>
      </AuthSessionContext.Provider>
    </QueryClientProvider>,
  )
}

async function selectGroupAndOption() {
  const groupInput = await screen.findByLabelText('Grupo de modificadores')
  fireEvent.focus(groupInput)
  fireEvent.click(await screen.findByRole('option', { name: /Leche · LECHE/ }))

  const optionInput = await screen.findByLabelText('Opción de modificador')
  fireEvent.focus(optionInput)
  fireEvent.click(await screen.findByRole('option', { name: /Leche normal · LECH-N/ }))
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('ModifierEffectsPage', () => {
  it('loads an absent config and saves an empty effects list', async () => {
    const backend = createBackend({ configStatus: 404, configCode: 'INVENTORY_MODIFIER_CONFIG_NOT_FOUND' })
    renderPage(backend)
    await selectGroupAndOption()

    expect(await screen.findByText('Esta opción no tiene efectos de inventario configurados.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Guardar efectos' }))

    await waitFor(() => expect(backend.puts.length).toBe(1))
    expect(backend.puts[0]).toEqual({ expectedVersion: 0, effects: [] })
  })

  it('preserves positive and negative deltas as exact strings', async () => {
    const backend = createBackend({
      config: {
        id: 'e-1', organizationId: ORG, modifierOptionId: 'o-1', version: 4, createdAt: 'x', updatedAt: 'x',
        effects: [
          { inventoryItemId: 'item-milk', code: 'LEC-1', name: 'Leche normal', baseUnit: 'MILLILITER', fractionalUnitAllowed: true, status: 'ACTIVE', quantityDelta: '-250.000000' },
          { inventoryItemId: 'item-oat', code: 'AVE-1', name: 'Bebida de avena', baseUnit: 'MILLILITER', fractionalUnitAllowed: true, status: 'ACTIVE', quantityDelta: '250.000000' },
        ],
      },
    })
    renderPage(backend)
    await selectGroupAndOption()

    await screen.findByText(/Leche normal/)
    fireEvent.click(screen.getByRole('button', { name: 'Guardar efectos' }))

    await waitFor(() => expect(backend.puts.length).toBe(1))
    expect(backend.puts[0]).toEqual({
      expectedVersion: 4,
      effects: [
        { inventoryItemId: 'item-milk', quantityDelta: '-250' },
        { inventoryItemId: 'item-oat', quantityDelta: '250' },
      ],
    })
  })

  it('rejects a zero delta before submitting', async () => {
    const backend = createBackend({
      config: {
        id: 'e-1', organizationId: ORG, modifierOptionId: 'o-1', version: 1, createdAt: 'x', updatedAt: 'x',
        effects: [{ inventoryItemId: 'item-milk', code: 'LEC-1', name: 'Leche normal', baseUnit: 'MILLILITER', fractionalUnitAllowed: true, status: 'ACTIVE', quantityDelta: '-250.000000' }],
      },
    })
    renderPage(backend)
    await selectGroupAndOption()

    const delta = await screen.findByLabelText('Cambio de consumo')
    fireEvent.change(delta, { target: { value: '0' } })
    fireEvent.click(screen.getByRole('button', { name: 'Guardar efectos' }))

    const alerts = await screen.findAllByRole('alert')
    expect(alerts.some((element) => element.textContent?.includes('El valor no puede ser cero.'))).toBe(true)
    expect(backend.puts.length).toBe(0)
  })

  it('prevents selecting the same item twice', async () => {
    const backend = createBackend({ configStatus: 404, configCode: 'INVENTORY_MODIFIER_CONFIG_NOT_FOUND' })
    renderPage(backend)
    await selectGroupAndOption()

    await screen.findByText('Esta opción no tiene efectos de inventario configurados.')
    fireEvent.click(screen.getByRole('button', { name: 'Añadir efecto' }))
    const firstPicker = screen.getByLabelText('Artículo')
    fireEvent.focus(firstPicker)
    fireEvent.click(await screen.findByRole('option', { name: /Harina 1kg/ }))

    fireEvent.click(screen.getByRole('button', { name: 'Añadir efecto' }))
    const secondPicker = screen.getAllByLabelText('Artículo')[1]
    fireEvent.focus(secondPicker)
    const reuse = await screen.findByRole('option', { name: /Harina 1kg/ })
    expect(reuse.getAttribute('aria-disabled')).toBe('true')
  })

  it('keeps an existing inactive effect visible', async () => {
    const backend = createBackend({
      config: {
        id: 'e-1', organizationId: ORG, modifierOptionId: 'o-1', version: 2, createdAt: 'x', updatedAt: 'x',
        effects: [{ inventoryItemId: 'item-old', code: 'CAF-1', name: 'Café en grano', baseUnit: 'GRAM', fractionalUnitAllowed: true, status: 'INACTIVE', quantityDelta: '-100.000000' }],
      },
    })
    renderPage(backend)
    await selectGroupAndOption()

    const picker = (await screen.findByLabelText('Artículo')) as HTMLInputElement
    await waitFor(() => expect(picker.value).toBe('Café en grano · CAF-1'))
  })

  it('handles a concurrent modification on save', async () => {
    const backend = createBackend({
      config: {
        id: 'e-1', organizationId: ORG, modifierOptionId: 'o-1', version: 2, createdAt: 'x', updatedAt: 'x',
        effects: [{ inventoryItemId: 'item-milk', code: 'LEC-1', name: 'Leche normal', baseUnit: 'MILLILITER', fractionalUnitAllowed: true, status: 'ACTIVE', quantityDelta: '-250.000000' }],
      },
      putStatus: 409, putCode: 'CONCURRENT_MODIFICATION',
    })
    renderPage(backend)
    await selectGroupAndOption()

    await screen.findByLabelText('Cambio de consumo')
    fireEvent.click(screen.getByRole('button', { name: 'Guardar efectos' }))

    const banner = await screen.findByRole('status')
    expect(banner.textContent).toContain('Los efectos cambiaron mientras los editabas')
  })

  it('shows a controlled message when the effects read is forbidden', async () => {
    const backend = createBackend({ configStatus: 403, configCode: 'FORBIDDEN' })
    renderPage(backend)
    await selectGroupAndOption()

    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toContain('No tienes permiso para administrar efectos de inventario.')
  })
})
