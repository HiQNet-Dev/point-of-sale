// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { AuthSessionContext } from '../../auth/model/authSessionContext'
import type { AuthSessionValue } from '../../auth/model/authSessionContext'
import type { AuthMe } from '../../auth/api/authApi'
import type { InventoryNegativeStockPolicy, Organization } from '../api/organizationApi'
import { InventorySettingsPage } from './InventorySettingsPage'

const ORG = 'org-1'

function organization(policy: InventoryNegativeStockPolicy, version: number): Organization {
  return {
    id: ORG,
    name: 'HiQNet POS Development',
    code: 'pos-dev',
    timezone: 'America/Mexico_City',
    currency: 'MXN',
    locale: 'es-MX',
    status: 'ACTIVE',
    inventoryNegativeStockPolicy: policy,
    version,
    createdAt: '2026-10-01T00:00:00Z',
    updatedAt: '2026-10-01T00:00:00Z',
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

interface BackendOptions {
  policy?: InventoryNegativeStockPolicy
  version?: number
  getStatus?: number
  getCode?: string
  patchStatus?: number
  patchCode?: string
  /** Estado que otro actor escribió antes de un PATCH en conflicto (devuelto por el refetch). */
  external?: { policy: InventoryNegativeStockPolicy; version: number }
}

function createBackend(options: BackendOptions = {}) {
  let current = organization(options.policy ?? 'STRICT', options.version ?? 5)
  const getRequests: URL[] = []
  const patchBodies: Array<Record<string, unknown>> = []

  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input), 'http://localhost')
    const method = init?.method ?? 'GET'
    if (url.pathname.endsWith(`/organizations/${ORG}`) && method === 'GET') {
      getRequests.push(url)
      if (options.getStatus) {
        return json({ code: options.getCode ?? 'ERROR', message: 'x' }, options.getStatus)
      }
      return json(current)
    }
    if (url.pathname.endsWith(`/organizations/${ORG}`) && method === 'PATCH') {
      const body = JSON.parse(String(init?.body)) as Record<string, unknown>
      patchBodies.push(body)
      if (options.patchStatus) {
        if (options.external) current = organization(options.external.policy, options.external.version)
        return json({ code: options.patchCode ?? 'ERROR', message: 'x' }, options.patchStatus)
      }
      current = {
        ...current,
        inventoryNegativeStockPolicy: body.inventoryNegativeStockPolicy as InventoryNegativeStockPolicy,
        version: current.version + 1,
      }
      return json(current)
    }
    return new Response(null, { status: 204 })
  })

  return { fetchMock, getRequests, patchBodies, current: () => current }
}

function renderPage(backend: ReturnType<typeof createBackend>) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  vi.stubGlobal('fetch', backend.fetchMock)
  render(
    <QueryClientProvider client={queryClient}>
      <AuthSessionContext.Provider value={authValue()}>
        <MemoryRouter initialEntries={['/admin/inventory/settings']}>
          <Routes>
            <Route path="/admin/inventory/settings" element={<InventorySettingsPage />} />
          </Routes>
        </MemoryRouter>
      </AuthSessionContext.Provider>
    </QueryClientProvider>,
  )
  return queryClient
}

function radio(name: RegExp): HTMLInputElement {
  return screen.getByRole('radio', { name }) as HTMLInputElement
}

function saveButton(): HTMLButtonElement {
  return screen.getByRole('button', { name: 'Guardar configuración' }) as HTMLButtonElement
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('InventorySettingsPage', () => {
  it('loads the organization policy scoped to the organization (no branch context)', async () => {
    const backend = createBackend({ policy: 'STRICT', version: 5 })
    renderPage(backend)

    await screen.findByRole('radio', { name: /No permitir stock negativo/ })
    expect(backend.getRequests[0].pathname).toBe(`/api/v1/organizations/${ORG}`)
    expect(backend.getRequests[0].search).toBe('')
    expect(backend.getRequests[0].pathname).not.toContain('branch')
  })

  it('shows STRICT as the selected policy', async () => {
    const backend = createBackend({ policy: 'STRICT' })
    renderPage(backend)

    await screen.findByRole('radio', { name: /No permitir stock negativo/ })
    expect(radio(/No permitir stock negativo/).checked).toBe(true)
    expect(radio(/Permitir stock negativo/).checked).toBe(false)
  })

  it('shows ALLOW_NEGATIVE as the selected policy', async () => {
    const backend = createBackend({ policy: 'ALLOW_NEGATIVE' })
    renderPage(backend)

    await screen.findByRole('radio', { name: /Permitir stock negativo/ })
    expect(radio(/Permitir stock negativo/).checked).toBe(true)
  })

  it('saves STRICT → ALLOW_NEGATIVE with the expected version and only the policy field', async () => {
    const backend = createBackend({ policy: 'STRICT', version: 5 })
    renderPage(backend)
    await screen.findByRole('radio', { name: /No permitir stock negativo/ })

    fireEvent.click(radio(/Permitir stock negativo/))
    fireEvent.click(saveButton())

    await waitFor(() => expect(backend.patchBodies).toHaveLength(1))
    expect(backend.patchBodies[0]).toEqual({
      expectedVersion: 5,
      inventoryNegativeStockPolicy: 'ALLOW_NEGATIVE',
    })
    expect(await screen.findByText('Configuración guardada.')).toBeTruthy()
  })

  it('saves ALLOW_NEGATIVE → STRICT', async () => {
    const backend = createBackend({ policy: 'ALLOW_NEGATIVE', version: 7 })
    renderPage(backend)
    await screen.findByRole('radio', { name: /Permitir stock negativo/ })

    fireEvent.click(radio(/No permitir stock negativo/))
    fireEvent.click(saveButton())

    await waitFor(() => expect(backend.patchBodies).toHaveLength(1))
    expect(backend.patchBodies[0]).toEqual({
      expectedVersion: 7,
      inventoryNegativeStockPolicy: 'STRICT',
    })
  })

  it('disables save when the selection equals the persisted policy (no-op)', async () => {
    const backend = createBackend({ policy: 'STRICT' })
    renderPage(backend)
    await screen.findByRole('radio', { name: /No permitir stock negativo/ })

    expect(saveButton().disabled).toBe(true)
    fireEvent.click(saveButton())
    expect(backend.patchBodies).toHaveLength(0)
  })

  it('handles a concurrent modification by refetching and showing the latest server policy', async () => {
    const backend = createBackend({
      policy: 'STRICT',
      version: 5,
      patchStatus: 409,
      patchCode: 'CONCURRENT_MODIFICATION',
      external: { policy: 'ALLOW_NEGATIVE', version: 6 },
    })
    renderPage(backend)
    await screen.findByRole('radio', { name: /No permitir stock negativo/ })

    fireEvent.click(radio(/Permitir stock negativo/))
    fireEvent.click(saveButton())

    expect(
      await screen.findByText(
        'La configuración de la organización cambió mientras la editabas. Actualizamos los datos; revisa la política e intenta nuevamente.',
      ),
    ).toBeTruthy()
    await waitFor(() => expect(backend.getRequests.length).toBeGreaterThan(1))
    expect(radio(/Permitir stock negativo/).checked).toBe(true)
    expect(screen.queryByText('Configuración guardada.')).toBeNull()
  })

  it('renders a controlled error when the organization cannot be read (403)', async () => {
    const backend = createBackend({ getStatus: 403, getCode: 'FORBIDDEN' })
    renderPage(backend)

    expect(await screen.findByText('No tienes permiso para esta configuración de inventario.')).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Guardar configuración' })).toBeNull()
  })

  it('renders a controlled error when the save is forbidden (403)', async () => {
    const backend = createBackend({ policy: 'STRICT', patchStatus: 403, patchCode: 'FORBIDDEN' })
    renderPage(backend)
    await screen.findByRole('radio', { name: /No permitir stock negativo/ })

    fireEvent.click(radio(/Permitir stock negativo/))
    fireEvent.click(saveButton())

    expect(await screen.findByText('No tienes permiso para esta configuración de inventario.')).toBeTruthy()
    expect(screen.queryByText('Configuración guardada.')).toBeNull()
  })

  it('renders a controlled error for a missing organization (404) instead of defaulting to STRICT', async () => {
    const backend = createBackend({ getStatus: 404, getCode: 'ORGANIZATION_NOT_FOUND' })
    renderPage(backend)

    expect(await screen.findByText('La organización no está disponible.')).toBeTruthy()
    expect(screen.queryByRole('radio')).toBeNull()
  })

  it('keeps the success feedback visible after the organization version changes', async () => {
    const backend = createBackend({ policy: 'STRICT', version: 5 })
    const queryClient = renderPage(backend)
    await screen.findByRole('radio', { name: /No permitir stock negativo/ })

    fireEvent.click(radio(/Permitir stock negativo/))
    fireEvent.click(saveButton())

    expect(await screen.findByText('Configuración guardada.')).toBeTruthy()
    expect(backend.current().version).toBe(6)

    await queryClient.invalidateQueries()
    await waitFor(() => expect(backend.getRequests.length).toBeGreaterThan(1))
    expect(screen.getByText('Configuración guardada.')).toBeTruthy()
    expect(radio(/Permitir stock negativo/).checked).toBe(true)
  })
})
