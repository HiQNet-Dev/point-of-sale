// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { configureApiAuth } from '../../../shared/api/client'
import type { Branch, CashRegister, CashShift } from '../../pos-session/api/posSessionApi'
import { SalesWorkspace } from './SalesWorkspace'
import type { ProductConfiguration, SaleDetail, SaleSummary } from '../api/posSalesApi'

const orgId = 'org-1'
const branchId = 'branch-1'
const registerId = 'register-1'
const shiftId = 'shift-1'
const saleId = 'sale-1'
const productId = 'product-1'
const optionId = 'option-1'

const branch: Branch = {
  id: branchId,
  organizationId: orgId,
  name: 'Centro',
  code: 'CTR',
  timezone: 'America/Mexico_City',
  status: 'ACTIVE',
}
const register: CashRegister = {
  id: registerId,
  organizationId: orgId,
  branchId,
  code: 'CAJA-1',
  name: 'Caja principal',
  status: 'ACTIVE',
}
const shift: CashShift = {
  id: shiftId,
  organizationId: orgId,
  branchId,
  cashRegisterId: registerId,
  status: 'OPEN',
  openingAmount: 0,
  openedAt: '2026-10-11T12:00:00Z',
  version: 2,
}
const summary: SaleSummary = {
  id: saleId,
  organizationId: orgId,
  branchId,
  cashRegisterId: registerId,
  cashShiftId: shiftId,
  status: 'OPEN',
  totalAmount: '18.5',
  version: 3,
  createdAt: '2026-10-11T12:00:00Z',
  completedAt: null,
  cancelledAt: null,
}
const product = {
  id: productId,
  sku: 'COF-1',
  barcode: '7500001',
  name: 'Café de olla',
  description: null,
  unitPrice: 15,
  categoryId: 'category-1',
  status: 'ACTIVE' as const,
}
const configuration: ProductConfiguration = {
  product: {
    id: productId,
    sku: product.sku,
    name: product.name,
    unitPrice: product.unitPrice,
    categoryId: product.categoryId,
    status: 'ACTIVE',
  },
  groups: [{
    id: 'group-1',
    code: 'MILK',
    name: 'Tipo de leche',
    minSelections: 1,
    maxSelections: 1,
    status: 'ACTIVE',
    displayOrder: 1,
    options: [{
      id: optionId,
      code: 'WHOLE',
      name: 'Entera',
      priceDelta: 3.5,
      displayOrder: 1,
      status: 'ACTIVE',
    }],
  }],
}

function item(overrides: Partial<SaleDetail['items'][number]> = {}): SaleDetail['items'][number] {
  return {
    id: 'item-1',
    productId,
    productSkuSnapshot: 'OLD-COF-1',
    productNameSnapshot: 'Café histórico',
    baseUnitPrice: '15',
    modifierUnitDelta: '3.5',
    effectiveUnitPrice: '18.5',
    quantity: 1,
    lineTotal: '18.5',
    status: 'ACTIVE',
    position: 1,
    modifiers: [{
      id: 'modifier-1',
      modifierGroupId: 'group-1',
      modifierOptionId: optionId,
      groupNameSnapshot: 'Leche histórica',
      optionNameSnapshot: 'Entera histórica',
      priceDeltaSnapshot: '3.5',
      displayOrder: 1,
    }],
    ...overrides,
  }
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

function page<T>(items: T[], totalElements = items.length, pageNumber = 0) {
  return { items, page: pageNumber, size: 20, totalElements, totalPages: Math.ceil(totalElements / 20) }
}

interface HarnessOptions {
  initialSales?: SaleSummary[]
  initialDetail?: SaleDetail
  products?: typeof product[]
  configuration?: ProductConfiguration
  addError?: { status: number; code: string }
  addNetworkError?: boolean
  updateError?: { status: number; code: string }
  removeError?: { status: number; code: string }
  cancelError?: { status: number; code: string }
  failFirstSaleDetail?: boolean
  configurationError?: { status: number; code: string }
  parents?: { branch?: Branch; register?: CashRegister; shift?: CashShift }
  fillHeight?: boolean
}

function setup(options: HarnessOptions = {}) {
  let sales = [...(options.initialSales ?? [])]
  let detail = options.initialDetail ?? { sale: summary, items: [] }
  const products = options.products ?? [product]
  let failFirstSaleDetail = options.failFirstSaleDetail ?? false
  const requests: Array<{ url: URL; method: string; body: Record<string, unknown> | null }> = []
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input), 'http://localhost')
    const method = init?.method ?? 'GET'
    const body = init?.body ? JSON.parse(String(init.body)) as Record<string, unknown> : null
    requests.push({ url, method, body })

    if (url.pathname.endsWith('/sales') && method === 'GET') {
      const pageNumber = Number(url.searchParams.get('page') ?? '0')
      const status = url.searchParams.get('status')
      const matchingSales = status ? sales.filter((sale) => sale.status === status) : sales
      return json(page(matchingSales.slice(pageNumber * 20, pageNumber * 20 + 20), matchingSales.length, pageNumber))
    }
    if (url.pathname.endsWith('/sales') && method === 'POST') {
      const created = { ...summary, id: `sale-${sales.length + 1}`, totalAmount: '0', version: 0 }
      sales = [created, ...sales]
      detail = { sale: created, items: [] }
      return json(created, 201)
    }
    if (url.pathname.endsWith(`/sales/${detail.sale.id}/checkout`) && method === 'POST') {
      detail = { ...detail, sale: { ...detail.sale, status: 'COMPLETED', version: detail.sale.version + 1 } }
      sales = sales.filter((sale) => sale.id !== detail.sale.id)
      return json(detail)
    }
    const detailSaleId = url.pathname.match(/\/sales\/([^/]+)$/)?.[1]
    if (detailSaleId && method === 'GET') {
      const selected = sales.find((sale) => sale.id === detailSaleId)
      if (selected) {
        if (failFirstSaleDetail) {
          failFirstSaleDetail = false
          return json({ code: 'TEMPORARY_FAILURE', message: 'Try again' }, 503)
        }
        if (detail.sale.id !== selected.id) detail = { sale: selected, items: [] }
        return json(detail)
      }
    }
    if (url.pathname.endsWith(`/sales/${detail.sale.id}/items`) && method === 'POST') {
      if (options.addNetworkError) throw new TypeError('Network request failed')
      if (options.addError) {
        if (['SALE_NOT_OPEN', 'SALE_ALREADY_CANCELLED', 'SALE_ALREADY_COMPLETED'].includes(options.addError.code)) {
          const status = options.addError.code === 'SALE_ALREADY_COMPLETED' ? 'COMPLETED' : 'CANCELLED'
          detail = { ...detail, sale: { ...detail.sale, status } }
          sales = sales.map((sale) => sale.id === detail.sale.id ? detail.sale : sale)
        }
        return json({ code: options.addError.code, message: 'Rejected' }, options.addError.status)
      }
      detail = {
        sale: { ...detail.sale, totalAmount: '37', version: detail.sale.version + 1 },
        items: [item({ quantity: Number(body?.quantity), lineTotal: '37' })],
      }
      sales = sales.map((sale) => sale.id === detail.sale.id ? detail.sale : sale)
      return json(detail)
    }
    if (url.pathname.endsWith(`/items/item-1`) && method === 'PATCH') {
      if (options.updateError) {
        return json({ code: options.updateError.code, message: 'Rejected' }, options.updateError.status)
      }
      detail = {
        sale: { ...detail.sale, totalAmount: '37', version: detail.sale.version + 1 },
        items: [item({ quantity: Number(body?.quantity), lineTotal: '37' })],
      }
      sales = sales.map((sale) => sale.id === detail.sale.id ? detail.sale : sale)
      return json(detail)
    }
    if (url.pathname.endsWith(`/items/item-1/remove`) && method === 'POST') {
      if (options.removeError) {
        return json({ code: options.removeError.code, message: 'Rejected' }, options.removeError.status)
      }
      detail = {
        sale: { ...detail.sale, totalAmount: '0', version: detail.sale.version + 1 },
        items: [item({ status: 'REMOVED', lineTotal: '0' })],
      }
      return json(detail)
    }
    if (url.pathname.endsWith(`/sales/${detail.sale.id}/cancel`) && method === 'POST') {
      if (options.cancelError) {
        return json({ code: options.cancelError.code, message: 'Rejected' }, options.cancelError.status)
      }
      detail = { sale: { ...detail.sale, status: 'CANCELLED', cancelledAt: '2026-10-11T13:00:00Z' }, items: detail.items }
      sales = sales.filter((sale) => sale.id !== detail.sale.id)
      return json(detail)
    }
    if (url.pathname.endsWith('/pos/catalog/categories')) {
      return json(page([{ id: 'category-1', code: 'HOT', name: 'Bebidas calientes', status: 'ACTIVE' }]))
    }
    if (url.pathname.endsWith('/pos/catalog/products')) {
      const pageNumber = Number(url.searchParams.get('page') ?? '0')
      return json(page(products.slice(pageNumber * 20, pageNumber * 20 + 20), products.length, pageNumber))
    }
    if (url.pathname.endsWith(`/pos/catalog/products/${productId}/configuration`)) {
      if (options.configurationError) {
        return json({ code: options.configurationError.code, message: 'Unavailable' }, options.configurationError.status)
      }
      return json(options.configuration ?? configuration)
    }
    return json({ code: 'NOT_FOUND', message: 'Not found' }, 404)
  })

  vi.stubGlobal('fetch', fetchMock)
  configureApiAuth({
    getAccessToken: () => 'test-access',
    refreshAccessToken: async () => 'test-access',
  })
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  const view = render(
    <QueryClientProvider client={queryClient}>
      <SalesWorkspace
        organizationId={orgId}
        branch={options.parents?.branch ?? branch}
        register={options.parents?.register ?? register}
        shift={options.parents?.shift ?? shift}
        locale="es-MX"
        currency="MXN"
        fillHeight={options.fillHeight}
      />
    </QueryClientProvider>,
  )
  return { ...view, fetchMock, queryClient, requests }
}

afterEach(() => {
  cleanup()
  configureApiAuth(null)
  vi.unstubAllGlobals()
})

describe('POS sales workspace', () => {
  it('completes the selected Sale once, clears it, refreshes OPEN Sales, and does not create a replacement', async () => {
    const backend = setup({ initialSales: [summary], initialDetail: { sale: summary, items: [item()] } })
    await screen.findByRole('button', { name: 'Cobrar' })
    fireEvent.click(screen.getByRole('button', { name: 'Cobrar' }))
    await screen.findByRole('dialog', { name: 'Cobrar venta' })
    const method = screen.getByRole('combobox', { name: 'Método de pago' })
    fireEvent.mouseDown(method)
    const listbox = await screen.findByRole('listbox')
    fireEvent.click(within(listbox).getByRole('option', { name: 'Tarjeta' }))
    fireEvent.change(screen.getByRole('textbox', { name: 'Monto aplicado' }), { target: { value: '18.5' } })
    fireEvent.click(screen.getByRole('button', { name: 'Completar venta' }))

    await screen.findByText('Venta completada.')
    await waitFor(() => expect(backend.requests.filter(({ url, method }) => url.pathname.endsWith('/sales') && method === 'POST')).toHaveLength(0))
    const checkout = backend.requests.find(({ url, method }) => url.pathname.endsWith('/sales/sale-1/checkout') && method === 'POST')
    expect(checkout?.body).toEqual({ expectedSaleVersion: 3, payments: [{ method: 'CARD', amount: '18.5' }] })
    expect(screen.queryByRole('dialog', { name: 'Cobrar venta' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Nueva venta' })).toBeTruthy()
    expect(backend.requests.filter(({ url, method }) => url.pathname.endsWith('/sales') && method === 'GET').length).toBeGreaterThan(1)
  })

  it('completes a zero-total Sale with no payment rows at workspace level', async () => {
    const freeSale = { ...summary, totalAmount: '0.0000' }
    setup({ initialSales: [freeSale], initialDetail: { sale: freeSale, items: [item({ lineTotal: '0' })] } })
    fireEvent.click(await screen.findByRole('button', { name: 'Cobrar' }))
    await screen.findByText('Esta venta no requiere pagos. Puedes completarla con importe cero.')
    expect(screen.queryByRole('combobox', { name: 'Método de pago' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Completar venta' }))
    await screen.findByText('Venta completada.')
  })

  it('does not create a Sale until Nueva venta is explicitly selected', async () => {
    const backend = setup()
    expect(await screen.findByText('No hay ventas abiertas.')).toBeTruthy()
    expect(backend.requests.filter((request) => request.method === 'POST' && request.url.pathname.endsWith('/sales'))).toHaveLength(0)

    fireEvent.click(screen.getByRole('button', { name: 'Nueva venta' }))
    expect(await screen.findByText('Tu venta está vacía. Selecciona un producto para comenzar.')).toBeTruthy()
    expect(backend.requests.filter((request) => request.method === 'POST' && request.url.pathname.endsWith('/sales'))).toHaveLength(1)
    expect(backend.requests.some((request) => request.url.pathname.endsWith('/pos/catalog/products'))).toBe(true)
  })

  it('recovers a created Sale after its first detail request fails without creating a replacement', async () => {
    const backend = setup({ failFirstSaleDetail: true })
    expect(await screen.findByText('No hay ventas abiertas.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Nueva venta' }))
    expect(await screen.findByText('No se pudo cargar la venta.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Actualizar' }))
    expect(await screen.findByText('Tu venta está vacía. Selecciona un producto para comenzar.')).toBeTruthy()
    expect(backend.requests.filter((request) => request.method === 'POST' && request.url.pathname.endsWith('/sales'))).toHaveLength(1)
    expect(backend.requests.filter((request) => request.method === 'GET' && request.url.pathname.endsWith('/sales/sale-1'))).toHaveLength(2)
  })

  it('resumes the persisted Sale, renders snapshots and uses server version for add', async () => {
    const backend = setup({ initialSales: [summary], initialDetail: { sale: summary, items: [item()] } })
    expect(await screen.findByText(/Café histórico/)).toBeTruthy()
    expect(screen.getByText('Leche histórica: Entera histórica')).toBeTruthy()
    expect(screen.getByLabelText('Total de la venta').textContent).toContain('18.50')
    expect(backend.requests.some((request) => request.url.pathname.endsWith('/pos/catalog/products'))).toBe(true)
    expect(backend.requests.some((request) => request.method === 'POST' && request.url.pathname.endsWith('/sales'))).toBe(false)

    fireEvent.click(await screen.findByRole('button', { name: /Café de olla/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    fireEvent.click(await within(dialog).findByLabelText(/Entera/))
    fireEvent.change(within(dialog).getByLabelText('Cantidad'), { target: { value: '2' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Agregar a la venta' }))

    await waitFor(() => expect(backend.requests.some((request) => request.method === 'POST' && request.url.pathname.endsWith('/sales/sale-1/items'))).toBe(true))
    const addRequest = backend.requests.find((request) => request.method === 'POST' && request.url.pathname.endsWith('/sales/sale-1/items'))
    expect(addRequest?.body).toEqual({
      expectedSaleVersion: 3,
      productId,
      quantity: 2,
      modifierOptionIds: [optionId],
    })
    expect(await screen.findByText(/2\s*×\s*Café histórico/)).toBeTruthy()
  })

  it('shows add permission failures inside the active Product dialog without retrying or changing the Sale', async () => {
    const backend = setup({
      initialSales: [summary],
      addError: { status: 403, code: 'FORBIDDEN' },
    })
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    fireEvent.click(await within(dialog).findByLabelText(/Entera/))
    const addButton = within(dialog).getByRole('button', { name: 'Agregar a la venta' })
    act(() => {
      fireEvent.click(addButton)
      fireEvent.click(addButton)
    })

    const permissionAlert = await within(dialog).findByRole('alert')
    expect(permissionAlert.textContent).toContain('No tienes permiso para modificar esta venta.')
    expect(dialog.getAttribute('aria-modal')).toBe('true')
    expect(within(dialog).getByRole('button', { name: 'Agregar a la venta' }).hasAttribute('disabled')).toBe(false)
    expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/sales/sale-1/items'))).toHaveLength(1)
    expect(backend.requests.filter((request) => request.method === 'GET'
      && request.url.pathname.endsWith('/sales/sale-1'))).toHaveLength(1)

    fireEvent.click(addButton)
    await waitFor(() => expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/sales/sale-1/items'))).toHaveLength(2))
    await waitFor(() => expect(addButton.hasAttribute('disabled')).toBe(false))
    fireEvent.click(within(dialog).getByRole('button', { name: 'Volver' }))
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    const nextDialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    expect(within(nextDialog).queryByRole('alert')).toBeNull()
  })

  it('releases the item mutation guard after a successful add so another Product can be added', async () => {
    const backend = setup({ initialSales: [summary] })
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    let dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    fireEvent.click(await within(dialog).findByLabelText(/Entera/))
    fireEvent.click(within(dialog).getByRole('button', { name: 'Agregar a la venta' }))
    await waitFor(() => expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/sales/sale-1/items'))).toHaveLength(1))
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Agregar producto' })).toBeNull())

    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    fireEvent.click(await within(dialog).findByLabelText(/Entera/))
    fireEvent.click(within(dialog).getByRole('button', { name: 'Agregar a la venta' }))
    await waitFor(() => expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/sales/sale-1/items'))).toHaveLength(2))
  })

  it('releases the item mutation guard after a transport error for an intentional retry', async () => {
    const backend = setup({ initialSales: [summary], addNetworkError: true })
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    fireEvent.click(await within(dialog).findByLabelText(/Entera/))
    const addButton = within(dialog).getByRole('button', { name: 'Agregar a la venta' })
    fireEvent.click(addButton)
    expect(await within(dialog).findByRole('alert')).toBeTruthy()
    await waitFor(() => expect(addButton.hasAttribute('disabled')).toBe(false))

    fireEvent.click(addButton)
    await waitFor(() => expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/sales/sale-1/items'))).toHaveLength(2))
  })

  it('paginates the OPEN Sale collection and permits resuming multiple sales', async () => {
    const second = { ...summary, id: 'sale-2', totalAmount: '22' }
    const backend = setup({ initialSales: [summary, second] })
    expect(await screen.findByRole('button', { name: /Reanudar venta SALE-1/ })).toBeTruthy()
    expect(screen.getByRole('button', { name: /Reanudar venta SALE-2/ })).toBeTruthy()
    expect(backend.requests[0]?.url.searchParams.get('status')).toBe('OPEN')
    expect(backend.requests[0]?.url.searchParams.get('size')).toBe('20')

    fireEvent.click(screen.getByRole('button', { name: /Reanudar venta SALE-2/ }))
    await screen.findByRole('heading', { name: /SALE-2/ })
    expect(backend.requests.some((request) => request.url.pathname.endsWith('/sales/sale-2'))).toBe(true)
  })

  it('pages through more than twenty OPEN Sales', async () => {
    const sales = Array.from({ length: 21 }, (_, index) => ({
      ...summary,
      id: `sale-${index + 1}`,
    }))
    const backend = setup({ initialSales: sales })
    expect(await screen.findByRole('button', { name: 'Reanudar venta SALE-1' })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Más ventas' }))
    await waitFor(() => expect(backend.requests.some((request) => request.url.pathname.endsWith('/sales')
      && request.url.searchParams.get('page') === '1')).toBe(true))
    expect(await screen.findByRole('button', { name: 'Reanudar venta SALE-21' })).toBeTruthy()
  })

  it.each([
    ['Branch', { branch: { ...branch, status: 'INACTIVE' as const } }],
    ['Register', { register: { ...register, status: 'INACTIVE' as const } }],
  ] as const)('allows cancellation recovery with an inactive %s while disabling catalog and line mutations', async (_parentType, parents) => {
    const backend = setup({
      initialSales: [summary],
      initialDetail: { sale: summary, items: [item()] },
      parents,
    })
    expect(await screen.findByText(/Café histórico/)).toBeTruthy()
    expect(screen.getByText(/no permite agregar o cambiar productos/)).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Cancelar venta' }).hasAttribute('disabled')).toBe(false)
    expect(screen.getByRole('button', { name: 'Editar Café histórico' }).hasAttribute('disabled')).toBe(true)
    expect(backend.requests.some((request) => request.url.pathname.endsWith('/pos/catalog/products'))).toBe(false)

    fireEvent.click(screen.getByRole('button', { name: 'Cancelar venta' }))
    const dialog = await screen.findByRole('dialog', { name: 'Cancelar venta' })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancelar venta' }))
    await waitFor(() => expect(backend.requests.some((request) => request.method === 'POST' && request.url.pathname.endsWith('/sales/sale-1/cancel'))).toBe(true))
    const cancelRequest = backend.requests.find((request) => request.url.pathname.endsWith('/sales/sale-1/cancel'))
    expect(cancelRequest?.body).toEqual({ expectedVersion: 3 })
    expect(await screen.findByText('Venta cancelada. La venta permanece en el historial.')).toBeTruthy()
  })

  it('uses the current modifier configuration to edit and submits the current Sale version', async () => {
    const historical = item({ productNameSnapshot: 'Nombre antiguo' })
    const backend = setup({ initialSales: [summary], initialDetail: { sale: summary, items: [historical] } })
    expect(await screen.findByText(/Nombre antiguo/)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Editar Nombre antiguo' }))
    const dialog = await screen.findByRole('dialog', { name: 'Editar producto' })
    expect((await within(dialog).findByLabelText(/Entera/) as HTMLInputElement).checked).toBe(true)
    fireEvent.change(within(dialog).getByLabelText('Cantidad'), { target: { value: '2' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Guardar cambios' }))

    await waitFor(() => expect(backend.requests.some((request) => request.method === 'PATCH' && request.url.pathname.endsWith('/items/item-1'))).toBe(true))
    const updateRequest = backend.requests.find((request) => request.method === 'PATCH' && request.url.pathname.endsWith('/items/item-1'))
    expect(updateRequest?.body).toEqual({ expectedSaleVersion: 3, quantity: 2, modifierOptionIds: [optionId] })
  })

  it('shows edit permission failures inside the active edit dialog without retrying or updating the Sale', async () => {
    const backend = setup({
      initialSales: [summary],
      initialDetail: { sale: summary, items: [item()] },
      updateError: { status: 403, code: 'FORBIDDEN' },
    })
    expect(await screen.findByText(/Café histórico/)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Editar Café histórico' }))
    const dialog = await screen.findByRole('dialog', { name: 'Editar producto' })
    fireEvent.change(await within(dialog).findByLabelText('Cantidad'), { target: { value: '2' } })
    const saveButton = within(dialog).getByRole('button', { name: 'Guardar cambios' })
    act(() => {
      fireEvent.click(saveButton)
      fireEvent.click(saveButton)
    })

    const permissionAlert = await within(dialog).findByRole('alert')
    expect(permissionAlert.textContent).toContain('No tienes permiso para modificar esta venta.')
    expect(within(dialog).getByRole('button', { name: 'Guardar cambios' }).hasAttribute('disabled')).toBe(false)
    expect(backend.requests.filter((request) => request.method === 'PATCH'
      && request.url.pathname.endsWith('/items/item-1'))).toHaveLength(1)
    expect(backend.requests.filter((request) => request.method === 'GET'
      && request.url.pathname.endsWith('/sales/sale-1'))).toHaveLength(1)
    expect(screen.getByLabelText('Total de la venta').textContent).toContain('18.50')
  })

  it('requires explicit confirmation and a valid current selection when modifier configuration changed', async () => {
    const changedConfiguration: ProductConfiguration = {
      ...configuration,
      groups: [{
        ...configuration.groups[0],
        options: [{ ...configuration.groups[0].options[0], id: 'option-current', name: 'Avena' }],
      }],
    }
    const backend = setup({
      initialSales: [summary],
      initialDetail: { sale: summary, items: [item()] },
      configuration: changedConfiguration,
    })
    expect(await screen.findByText(/Café histórico/)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Editar Café histórico' }))
    const dialog = await screen.findByRole('dialog', { name: 'Editar producto' })
    expect(await within(dialog).findByText(/La configuración cambió/)).toBeTruthy()
    const saveButton = within(dialog).getByRole('button', { name: 'Guardar cambios' })
    expect(saveButton.hasAttribute('disabled')).toBe(true)
    fireEvent.click(within(dialog).getByRole('button', { name: 'Usar opciones activas' }))
    fireEvent.click(await within(dialog).findByLabelText(/Avena/))
    fireEvent.click(saveButton)

    await waitFor(() => expect(backend.requests.some((request) => request.method === 'PATCH'
      && request.url.pathname.endsWith('/items/item-1'))).toBe(true))
    expect(backend.requests.find((request) => request.method === 'PATCH' && request.url.pathname.endsWith('/items/item-1'))?.body)
      .toEqual({ expectedSaleVersion: 3, quantity: 1, modifierOptionIds: ['option-current'] })
  })

  it('uses the remove command and expected Sale version; removed lines leave the active cart', async () => {
    const backend = setup({ initialSales: [summary], initialDetail: { sale: summary, items: [item()] } })
    expect(await screen.findByText(/Café histórico/)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Quitar Café histórico' }))
    const dialog = await screen.findByRole('dialog', { name: 'Quitar producto' })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Quitar' }))

    await waitFor(() => expect(backend.requests.some((request) => request.method === 'POST' && request.url.pathname.endsWith('/items/item-1/remove'))).toBe(true))
    const removeRequest = backend.requests.find((request) => request.url.pathname.endsWith('/items/item-1/remove'))
    expect(removeRequest?.body).toEqual({ expectedVersion: 3 })
    expect(await screen.findByText('Elementos retirados (1)')).toBeTruthy()
    expect(screen.getByText(/Café histórico.*Retirado/)).toBeTruthy()
    expect(screen.getByText('Tu venta está vacía. Selecciona un producto para comenzar.')).toBeTruthy()
  })

  it('shows remove permission failures inside the active confirmation dialog without retrying', async () => {
    const backend = setup({
      initialSales: [summary],
      initialDetail: { sale: summary, items: [item()] },
      removeError: { status: 403, code: 'FORBIDDEN' },
    })
    expect(await screen.findByText(/Café histórico/)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Quitar Café histórico' }))
    const dialog = await screen.findByRole('dialog', { name: 'Quitar producto' })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Quitar' }))

    expect((await within(dialog).findByRole('alert')).textContent).toContain('No tienes permiso para modificar esta venta.')
    expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/items/item-1/remove'))).toHaveLength(1)
    expect(dialog.getAttribute('aria-modal')).toBe('true')
  })

  it('shows cancel permission failures inside the active confirmation dialog without retrying', async () => {
    const backend = setup({
      initialSales: [summary],
      initialDetail: { sale: summary, items: [item()] },
      cancelError: { status: 403, code: 'FORBIDDEN' },
    })
    expect(await screen.findByText(/Café histórico/)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Cancelar venta' }))
    const dialog = await screen.findByRole('dialog', { name: 'Cancelar venta' })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancelar venta' }))

    expect((await within(dialog).findByRole('alert')).textContent).toContain('No tienes permiso para modificar esta venta.')
    expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/sales/sale-1/cancel'))).toHaveLength(1)
    expect(dialog.getAttribute('aria-modal')).toBe('true')
  })

  it('sends search/category filters and can reach later Product pages', async () => {
    const products = Array.from({ length: 21 }, (_, index) => ({
      ...product,
      id: `product-${index}`,
      sku: `SKU-${String(index).padStart(2, '0')}`,
      name: `Bebida ${index}`,
    }))
    const backend = setup({ initialSales: [summary], products })
    expect(await screen.findByText('Bebida 0')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Más productos' }))
    await waitFor(() => expect(backend.requests.some((request) => request.url.pathname.endsWith('/pos/catalog/products')
      && request.url.searchParams.get('page') === '1')).toBe(true))
    expect(await screen.findByText('Bebida 20')).toBeTruthy()

    fireEvent.change(screen.getByRole('textbox', { name: 'Buscar productos' }), { target: { value: 'latte' } })
    await waitFor(() => expect(backend.requests.some((request) => request.url.pathname.endsWith('/pos/catalog/products')
      && request.url.searchParams.get('q') === 'latte' && request.url.searchParams.get('page') === '0')).toBe(true), { timeout: 1500 })
    fireEvent.click(screen.getByRole('button', { name: 'Bebidas calientes' }))
    await waitFor(() => expect(backend.requests.some((request) => request.url.pathname.endsWith('/pos/catalog/products')
      && request.url.searchParams.get('categoryId') === 'category-1')).toBe(true))
  })

  it('enfoca la búsqueda de productos con el atajo F2 en el modo caja', async () => {
    setup({ initialSales: [summary] })
    const search = await screen.findByRole('textbox', { name: 'Buscar productos' })
    expect(search.getAttribute('aria-keyshortcuts')).toBe('F2')

    fireEvent.keyDown(window, { key: 'F2' })
    expect(document.activeElement).toBe(search)
  })

  it('ignora F2 cuando el foco está en un campo editable', async () => {
    setup({ initialSales: [summary] })
    const search = await screen.findByRole('textbox', { name: 'Buscar productos' })
    const productButton = await screen.findByRole('button', { name: /Café de olla/ })
    productButton.focus()
    expect(document.activeElement).toBe(productButton)

    fireEvent.keyDown(search, { key: 'F2' })
    expect(document.activeElement).toBe(productButton)
  })

  it('ignora F2 mientras hay un diálogo abierto', async () => {
    setup({ initialSales: [summary] })
    const search = await screen.findByRole('textbox', { name: 'Buscar productos' })
    fireEvent.click(await screen.findByRole('button', { name: /Café de olla/ }))
    await screen.findByRole('dialog', { name: 'Agregar producto' })

    fireEvent.keyDown(window, { key: 'F2' })
    expect(document.activeElement).not.toBe(search)
  })

  it('mantiene el total y el cobro visibles en el panel del carrito con altura completa', async () => {
    setup({ initialSales: [summary], initialDetail: { sale: summary, items: [item()] }, fillHeight: true })
    expect(await screen.findByRole('button', { name: 'Cobrar' })).toBeTruthy()
    expect(screen.getByLabelText('Total de la venta').textContent).toContain('18.50')
    expect(screen.getByText(/Café histórico/)).toBeTruthy()
  })

  it('refreshes Product discovery when configuration reports that Product became inactive', async () => {
    const backend = setup({
      initialSales: [summary],
      configurationError: { status: 409, code: 'PRODUCT_INACTIVE' },
    })
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    expect(await within(dialog).findByText('Este producto dejó de estar disponible. Cierra la configuración y actualiza el catálogo.')).toBeTruthy()
    expect(within(dialog).getByRole('button', { name: 'Agregar a la venta' }).hasAttribute('disabled')).toBe(true)
    await waitFor(() => expect(backend.requests.filter((request) => request.url.pathname.endsWith('/pos/catalog/products'))).toHaveLength(2))
  })

  it('blocks a product with an inactive required group and keeps inactive options unavailable', async () => {
    const unavailableConfiguration: ProductConfiguration = {
      ...configuration,
      groups: [
        { ...configuration.groups[0], status: 'INACTIVE', options: [{ ...configuration.groups[0].options[0], status: 'INACTIVE' }] },
        { ...configuration.groups[0], id: 'optional-inactive', name: 'Grupo opcional oculto', minSelections: 0, status: 'INACTIVE' },
      ],
    }
    setup({ initialSales: [summary], configuration: unavailableConfiguration })
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    expect(await within(dialog).findByText('Producto temporalmente no disponible por su configuración.')).toBeTruthy()
    expect(screen.queryByText('Grupo opcional oculto')).toBeNull()
    expect(within(dialog).getByRole('button', { name: 'Agregar a la venta' }).hasAttribute('disabled')).toBe(true)
    expect((within(dialog).getByLabelText(/No disponible/) as HTMLInputElement).disabled).toBe(true)
  })

  it('reconciles an externally completed Sale after an add terminal-state conflict', async () => {
    const backend = setup({
      initialSales: [summary],
      addError: { status: 409, code: 'SALE_ALREADY_COMPLETED' },
    })
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    fireEvent.click(await within(dialog).findByLabelText(/Entera/))
    fireEvent.click(within(dialog).getByRole('button', { name: 'Agregar a la venta' }))
    expect(await screen.findByText('La venta ya no está abierta. Se actualizará la lista de ventas.')).toBeTruthy()
    expect(await screen.findByText('No hay ventas abiertas.')).toBeTruthy()
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Agregar producto' })).toBeNull())
    expect(backend.requests.filter((request) => request.method === 'GET' && request.url.pathname.endsWith('/sales/sale-1'))).toHaveLength(2)
    expect(backend.requests.filter((request) => request.method === 'POST' && request.url.pathname.endsWith('/sales/sale-1/items'))).toHaveLength(1)
  })

  it('refreshes configuration while surfacing a modifier-configuration mutation conflict in the dialog', async () => {
    const backend = setup({
      initialSales: [summary],
      addError: { status: 409, code: 'PRODUCT_MODIFIER_CONFIGURATION_UNAVAILABLE' },
    })
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    fireEvent.click(await within(dialog).findByLabelText(/Entera/))
    fireEvent.click(within(dialog).getByRole('button', { name: 'Agregar a la venta' }))

    expect((await within(dialog).findByRole('alert')).textContent)
      .toContain('La configuración cambió y ya no permite agregar este producto.')
    await waitFor(() => expect(backend.requests.filter((request) => request.url.pathname.endsWith('/configuration'))).toHaveLength(2))
    expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/sales/sale-1/items'))).toHaveLength(1)
  })

  it('reconciles a Product that becomes inactive during the add mutation', async () => {
    const backend = setup({
      initialSales: [summary],
      addError: { status: 409, code: 'PRODUCT_INACTIVE' },
    })
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    fireEvent.click(await within(dialog).findByLabelText(/Entera/))
    fireEvent.click(within(dialog).getByRole('button', { name: 'Agregar a la venta' }))

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Agregar producto' })).toBeNull())
    expect(await screen.findByText('El producto dejó de estar disponible. Se actualizará el catálogo.')).toBeTruthy()
    await waitFor(() => expect(backend.requests.filter((request) => request.url.pathname.endsWith('/pos/catalog/products'))).toHaveLength(2))
    expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/sales/sale-1/items'))).toHaveLength(1)
  })

  it('does not retry a stale add and refetches Sale detail after conflict', async () => {
    const backend = setup({ initialSales: [summary], addError: { status: 409, code: 'CONCURRENT_MODIFICATION' } })
    expect(await screen.findByRole('button', { name: /Café de olla/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Café de olla/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Agregar producto' })
    fireEvent.click(await within(dialog).findByLabelText(/Entera/))
    fireEvent.click(within(dialog).getByRole('button', { name: 'Agregar a la venta' }))
    const conflictAlert = await within(dialog).findByRole('alert')
    expect(conflictAlert.textContent).toContain('La venta cambió en otra operación')
    expect(backend.requests.filter((request) => request.method === 'POST'
      && request.url.pathname.endsWith('/sales/sale-1/items'))).toHaveLength(1)
    expect(backend.requests.filter((request) => request.method === 'GET'
      && request.url.pathname.endsWith('/sales/sale-1'))).toHaveLength(2)
  })
})
