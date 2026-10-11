// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Navigate, createMemoryRouter, RouterProvider } from 'react-router-dom'
import { LoginRoute } from '../../auth/components/LoginRoute'
import { ProtectedRoute } from '../../auth/components/ProtectedRoute'
import { AuthSessionProvider } from '../../auth/model/AuthSessionProvider'
import { authTokenStore } from '../../auth/model/authTokenStore'
import { configureApiAuth } from '../../../shared/api/client'
import { LoginPage } from '../../auth/pages/LoginPage'
import type { AuthMe, TokenResponse } from '../../auth/api/authApi'
import type { Branch, CashRegister, CashShift, PageResponse } from '../api/posSessionApi'
import { PosSessionPage } from './PosSessionPage'

const ORGANIZATION_ID = 'org-discovered-in-me'
const BRANCH_A_ID = 'branch-discovered-a'
const BRANCH_B_ID = 'branch-discovered-b'
const REGISTER_A_ID = 'register-discovered-a'
const REGISTER_B_ID = 'register-discovered-b'
const SHIFT_ID = 'shift-discovered-open'

const branchA: Branch = {
  id: BRANCH_A_ID,
  organizationId: ORGANIZATION_ID,
  name: 'Centro',
  code: 'CTR',
  timezone: 'America/Mexico_City',
  status: 'ACTIVE',
}
const registerA: CashRegister = {
  id: REGISTER_A_ID,
  organizationId: ORGANIZATION_ID,
  branchId: BRANCH_A_ID,
  code: 'CAJA-1',
  name: 'Caja principal',
  status: 'ACTIVE',
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

function page<T>(items: T[], totalElements = items.length, pageNumber = 0): PageResponse<T> {
  return { items, page: pageNumber, size: 20, totalElements, totalPages: Math.ceil(totalElements / 20) }
}

function tokens(): TokenResponse {
  return {
    accessToken: 'access-pos-test',
    tokenType: 'Bearer',
    expiresIn: 600,
    refreshToken: 'refresh-pos-test',
    refreshExpiresAt: '2026-11-07T00:00:00Z',
  }
}

function me(): AuthMe {
  return {
    userId: 'user-authenticated',
    organizationId: ORGANIZATION_ID,
    email: 'operator@example.com',
    status: 'ACTIVE',
    organization: { currency: 'MXN', locale: 'es-MX', timezone: 'America/Mexico_City' },
  }
}

interface BackendOptions {
  branches?: Branch[]
  registers?: CashRegister[]
  openShift?: CashShift | null
  registersStatus?: number
  shiftsStatus?: number
  closeStatus?: number
  openStatus?: number
}

function createBackend(options: BackendOptions = {}) {
  const branches = options.branches ?? [branchA]
  const registers = options.registers ?? [registerA]
  let activeShift = options.openShift ?? null
  const branchRequests: URL[] = []
  const registerRequests: URL[] = []
  const shiftRequests: Array<{ url: URL; method: string; body: Record<string, unknown> | null }> = []

  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input), 'http://localhost')
    const method = init?.method ?? 'GET'
    if (url.pathname.endsWith('/auth/login')) return json(tokens())
    if (url.pathname.endsWith('/auth/me')) return json(me())
    if (url.pathname.endsWith('/auth/me/branches')) {
      branchRequests.push(url)
      const pageNumber = Number(url.searchParams.get('page') ?? 0)
      return json(page(branches.slice(pageNumber * 20, (pageNumber + 1) * 20), branches.length, pageNumber))
    }
    if (url.pathname.endsWith('/cash-registers')) {
      registerRequests.push(url)
      if (options.registersStatus) return json({ code: 'FORBIDDEN', message: 'Forbidden' }, options.registersStatus)
      const branchId = url.pathname.split('/branches/')[1]?.split('/')[0]
      const pageNumber = Number(url.searchParams.get('page') ?? 0)
      const scopedRegisters = registers.filter((register) => register.branchId === branchId)
      return json(page(scopedRegisters.slice(pageNumber * 20, (pageNumber + 1) * 20), scopedRegisters.length, pageNumber))
    }
    if (url.pathname.includes('/cash-shifts')) {
      const body = init?.body ? JSON.parse(String(init.body)) as Record<string, unknown> : null
      shiftRequests.push({ url, method, body })
      if (method === 'POST' && url.pathname.endsWith('/close')) {
        if (options.closeStatus) return json({ code: 'FORBIDDEN', message: 'Forbidden' }, options.closeStatus)
        if (options.openStatus) return json({ code: 'CONCURRENT_MODIFICATION', message: 'Changed' }, options.openStatus)
        const closed = activeShift ? { ...activeShift, status: 'CLOSED' as const } : null
        activeShift = null
        return json(closed)
      }
      if (method === 'POST') {
        if (options.openStatus) return json({ code: 'CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT', message: 'Already open' }, options.openStatus)
        activeShift = {
          id: SHIFT_ID,
          organizationId: ORGANIZATION_ID,
          branchId: BRANCH_A_ID,
          cashRegisterId: REGISTER_A_ID,
          status: 'OPEN',
          openingAmount: 100.25,
          openedAt: '2026-10-10T10:00:00Z',
          version: 0,
        }
        return json(activeShift, 201)
      }
      if (options.shiftsStatus) return json({ code: 'FORBIDDEN', message: 'Forbidden' }, options.shiftsStatus)
      const registerId = url.pathname.split('/cash-registers/')[1]?.split('/')[0]
      const scopedShift = activeShift?.cashRegisterId === registerId ? activeShift : null
      return json(page(scopedShift ? [scopedShift] : []))
    }
    return new Response(null, { status: 204 })
  })

  return { fetchMock, branchRequests, registerRequests, shiftRequests }
}

function renderPosApp(backend: ReturnType<typeof createBackend>) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  const router = createMemoryRouter([
    { path: '/login', element: <LoginRoute><LoginPage /></LoginRoute> },
    { path: '/pos', element: <ProtectedRoute><PosSessionPage /></ProtectedRoute> },
    { path: '*', element: <Navigate to="/pos" replace /> },
  ], { initialEntries: ['/pos'] })
  vi.stubGlobal('fetch', backend.fetchMock)
  const view = render(
    <QueryClientProvider client={queryClient}>
      <AuthSessionProvider><RouterProvider router={router} /></AuthSessionProvider>
    </QueryClientProvider>,
  )
  return { ...view, router, queryClient }
}

async function loginToPos() {
  await screen.findByRole('heading', { name: 'Sign in' })
  fireEvent.change(screen.getByLabelText(/Organization code/), { target: { value: 'cafeteria' } })
  fireEvent.change(screen.getByLabelText(/Email/), { target: { value: 'operator@example.com' } })
  fireEvent.change(screen.getByLabelText(/Password/), { target: { value: 'correct-password' } })
  fireEvent.click(screen.getByRole('button', { name: 'Sign in' }))
  await screen.findByRole('heading', { name: 'Punto de venta' })
}

async function chooseOption(label: string, option: RegExp) {
  fireEvent.mouseDown(await screen.findByRole('combobox', { name: label }))
  fireEvent.click(await screen.findByRole('option', { name: option }))
}

afterEach(() => {
  cleanup()
  authTokenStore.clear()
  configureApiAuth(null)
  vi.unstubAllGlobals()
})

describe('POS session operational vertical', () => {
  it('cold-recovers an OPEN shift on inactive branch and register, then closes using response-derived IDs/version', async () => {
    const inactiveBranch = { ...branchA, status: 'INACTIVE' as const }
    const inactiveRegister = { ...registerA, status: 'INACTIVE' as const }
    const openShift: CashShift = {
      id: SHIFT_ID,
      organizationId: ORGANIZATION_ID,
      branchId: BRANCH_A_ID,
      cashRegisterId: REGISTER_A_ID,
      status: 'OPEN',
      openingAmount: 500.125,
      openedAt: '2026-10-10T16:30:00Z',
      version: 7,
    }
    const backend = createBackend({ branches: [inactiveBranch], registers: [inactiveRegister], openShift })
    renderPosApp(backend)

    expect(await screen.findByRole('heading', { name: 'Sign in' })).toBeTruthy()
    await loginToPos()
    expect(await screen.findByText('TURNO ACTIVO')).toBeTruthy()
    expect(screen.getByRole('combobox', { name: 'Sucursal' }).textContent).toBe('Centro · CTR')
    expect(screen.getByRole('combobox', { name: 'Caja' }).textContent).toBe('Caja principal · CAJA-1')
    expect(screen.getByRole('button', { name: 'Cerrar turno' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Abrir turno' })).toBeNull()
    expect(backend.branchRequests[0]?.searchParams.has('status')).toBe(false)
    expect(backend.registerRequests[0]?.searchParams.has('status')).toBe(false)

    fireEvent.click(screen.getByRole('button', { name: 'Cerrar turno' }))
    fireEvent.change(screen.getByLabelText('Efectivo declarado'), { target: { value: '550.1250' } })
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Cerrar turno' }))
    await screen.findByText('Sin turno activo')

    const closeRequest = backend.shiftRequests.find((request) => request.url.pathname.endsWith(`/${SHIFT_ID}/close`))
    expect(closeRequest?.method).toBe('POST')
    expect(closeRequest?.url.pathname).toContain(`/organizations/${ORGANIZATION_ID}/branches/${BRANCH_A_ID}/cash-registers/${REGISTER_A_ID}/`)
    expect(closeRequest?.body).toEqual({ expectedVersion: 7, declaredCashAmount: '550.1250' })
    expect(backend.branchRequests).toHaveLength(1)
    expect(backend.registerRequests[0]?.pathname).toContain(`/branches/${BRANCH_A_ID}/cash-registers`)
    expect(backend.shiftRequests[0]?.url.searchParams.get('status')).toBe('OPEN')
    expect(backend.shiftRequests
      .filter((request) => request.method === 'GET')
      .every((request) => request.url.searchParams.get('status') === 'OPEN')).toBe(true)
  })

  it('discovers resources from responses, opens with exact decimal string, then refreshes OPEN shifts', async () => {
    const backend = createBackend()
    renderPosApp(backend)
    await loginToPos()

    expect(await screen.findByText('Sin turno activo')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Abrir turno' }).hasAttribute('disabled')).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: 'Abrir turno' }))
    fireEvent.change(screen.getByLabelText('Fondo inicial'), { target: { value: '0.0001' } })
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Abrir turno' }))

    expect(await screen.findByText('TURNO ACTIVO')).toBeTruthy()
    const openRequest = backend.shiftRequests.find((request) => request.method === 'POST' && request.url.pathname.endsWith('/cash-shifts'))
    expect(openRequest?.url.pathname).toContain(`/organizations/${ORGANIZATION_ID}/branches/${BRANCH_A_ID}/cash-registers/${REGISTER_A_ID}/cash-shifts`)
    expect(openRequest?.body).toEqual({ openingAmount: '0.0001' })
    expect(backend.shiftRequests.filter((request) => request.method === 'GET')).toHaveLength(2)
  })

  it('keeps a discovered branch visible when register read returns 403', async () => {
    const backend = createBackend({ registersStatus: 403 })
    renderPosApp(backend)
    await loginToPos()

    expect(await screen.findByText('Centro · CTR')).toBeTruthy()
    expect(await screen.findByText('No tienes permiso para consultar las cajas de esta sucursal.')).toBeTruthy()
    expect(backend.branchRequests).toHaveLength(1)
    expect(backend.registerRequests).toHaveLength(1)
    expect(backend.shiftRequests).toHaveLength(0)
  })

  it('keeps the selected register visible when shift read returns 403', async () => {
    const backend = createBackend({ shiftsStatus: 403 })
    renderPosApp(backend)
    await loginToPos()

    expect(await screen.findByText('Caja principal · CAJA-1')).toBeTruthy()
    expect(await screen.findByText('No tienes permiso para consultar los turnos de esta caja.')).toBeTruthy()
    expect(backend.registerRequests).toHaveLength(1)
    expect(backend.shiftRequests).toHaveLength(1)
  })

  it('resets register and shift context when switching between discovered branches', async () => {
    const branchB: Branch = {
      ...branchA,
      id: BRANCH_B_ID,
      name: 'Norte',
      code: 'NTE',
      status: 'ACTIVE',
    }
    const registerB: CashRegister = {
      ...registerA,
      id: REGISTER_B_ID,
      branchId: BRANCH_B_ID,
      name: 'Caja secundaria',
      code: 'CAJA-2',
    }
    const openShift: CashShift = {
      id: SHIFT_ID,
      organizationId: ORGANIZATION_ID,
      branchId: BRANCH_A_ID,
      cashRegisterId: REGISTER_A_ID,
      status: 'OPEN',
      openingAmount: 100,
      openedAt: '2026-10-10T16:30:00Z',
      version: 0,
    }
    const backend = createBackend({ branches: [branchA, branchB], registers: [registerA, registerB], openShift })
    renderPosApp(backend)
    await loginToPos()

    await chooseOption('Sucursal', /Centro · CTR/)
    expect(await screen.findByText('TURNO ACTIVO')).toBeTruthy()
    await chooseOption('Sucursal', /Norte · NTE/)

    await waitFor(() => expect(backend.registerRequests.some((url) => url.pathname.includes(`/branches/${BRANCH_B_ID}/`))).toBe(true))
    expect(screen.getByRole('combobox', { name: 'Sucursal' }).textContent).toBe('Norte · NTE')
    expect(await screen.findByRole('combobox', { name: 'Caja' })).toBeTruthy()
    expect(screen.getByRole('combobox', { name: 'Caja' }).textContent).toBe('Caja secundaria · CAJA-2')
    expect(backend.shiftRequests.some((request) => request.url.pathname.includes(`/branches/${BRANCH_B_ID}/cash-registers/${REGISTER_B_ID}/`))).toBe(true)
    expect(screen.queryByText('TURNO ACTIVO')).toBeNull()
  })

  it('shows an empty self-scoped branch result instead of an error', async () => {
    const backend = createBackend({ branches: [] })
    renderPosApp(backend)
    await loginToPos()

    expect(await screen.findByText('No tienes sucursales asignadas.')).toBeTruthy()
    expect(backend.registerRequests).toHaveLength(0)
  })

  it.each([
    {
      label: 'branch',
      branch: { ...branchA, status: 'INACTIVE' as const },
      register: registerA,
      warning: 'Esta sucursal está inactiva. No se pueden abrir nuevos turnos.',
    },
    {
      label: 'register',
      branch: branchA,
      register: { ...registerA, status: 'INACTIVE' as const },
      warning: 'Esta caja está inactiva. No se pueden abrir nuevos turnos.',
    },
  ])('does not open when the $label is inactive without an OPEN shift', async ({ branch, register, warning }) => {
    const backend = createBackend({ branches: [branch], registers: [register] })
    renderPosApp(backend)
    await loginToPos()

    expect(await screen.findByText('Sin turno activo')).toBeTruthy()
    expect(screen.getByText(warning)).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Abrir turno' }).hasAttribute('disabled')).toBe(true)
    expect(backend.shiftRequests.some((request) => request.method === 'POST')).toBe(false)
  })

  it('reports the OPEN-shift invariant violation without selecting one silently', async () => {
    const firstShift = {
      id: SHIFT_ID,
      organizationId: ORGANIZATION_ID,
      branchId: BRANCH_A_ID,
      cashRegisterId: REGISTER_A_ID,
      status: 'OPEN' as const,
      openingAmount: 100,
      openedAt: '2026-10-10T16:30:00Z',
      version: 0,
    }
    const backend = createBackend({ openShift: firstShift })
    const originalFetch = backend.fetchMock.getMockImplementation()
    backend.fetchMock.mockImplementation(async (input, init) => {
      const url = new URL(String(input), 'http://localhost')
      if (url.pathname.includes('/cash-shifts') && (init?.method ?? 'GET') === 'GET') {
        return json(page([firstShift, { ...firstShift, id: 'second-response-shift' }], 2))
      }
      return originalFetch!(input, init)
    })
    renderPosApp(backend)
    await loginToPos()

    expect(await screen.findByText(/Hay más de un turno OPEN/)).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Cerrar turno' })).toBeNull()
  })

  it('keeps an OPEN shift visible when close is forbidden', async () => {
    const openShift: CashShift = {
      id: SHIFT_ID,
      organizationId: ORGANIZATION_ID,
      branchId: BRANCH_A_ID,
      cashRegisterId: REGISTER_A_ID,
      status: 'OPEN',
      openingAmount: 100,
      openedAt: '2026-10-10T16:30:00Z',
      version: 3,
    }
    const backend = createBackend({ openShift, closeStatus: 403 })
    renderPosApp(backend)
    await loginToPos()

    expect(await screen.findByText('TURNO ACTIVO')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Cerrar turno' }))
    fireEvent.change(screen.getByLabelText('Efectivo declarado'), { target: { value: '100' } })
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Cerrar turno' }))

    expect(await screen.findByText('No tienes permiso para cerrar este turno.')).toBeTruthy()
    expect(screen.getByText('TURNO ACTIVO')).toBeTruthy()
    expect(backend.shiftRequests.filter((request) => request.method === 'POST')).toHaveLength(1)
  })

  it('handles an already-open race by refetching OPEN shifts', async () => {
    const backend = createBackend({ openStatus: 409 })
    renderPosApp(backend)
    await loginToPos()
    await screen.findByText('Sin turno activo')
    fireEvent.click(screen.getByRole('button', { name: 'Abrir turno' }))
    fireEvent.change(screen.getByLabelText('Fondo inicial'), { target: { value: '25' } })
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Abrir turno' }))

    expect(await screen.findByText('La caja ya tiene un turno activo. Se actualizará su estado.')).toBeTruthy()
    expect(backend.shiftRequests.filter((request) => request.method === 'GET')).toHaveLength(2)
    expect(screen.getByText('Sin turno activo')).toBeTruthy()
  })

  it('paginates branches with default size and resets downstream context', async () => {
    const branches = Array.from({ length: 21 }, (_, index) => ({
      ...branchA,
      id: `branch-response-${index}`,
      name: `Branch ${index}`,
      code: `B-${index}`,
    }))
    const backend = createBackend({ branches })
    renderPosApp(backend)
    await loginToPos()

    await chooseOption('Sucursal', /Branch 0 · B-0/)
    expect(backend.branchRequests[0]?.searchParams.get('page')).toBe('0')
    expect(backend.branchRequests[0]?.searchParams.get('size')).toBe('20')
    fireEvent.click(screen.getByRole('button', { name: 'Página siguiente de sucursales' }))
    await waitFor(() => expect(backend.branchRequests.some((url) => url.searchParams.get('page') === '1')).toBe(true))
    fireEvent.mouseDown(await screen.findByRole('combobox', { name: 'Sucursal' }))
    expect(await screen.findByRole('option', { name: /Branch 20 · B-20/ })).toBeTruthy()
  })
})
