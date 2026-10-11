// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { configureApiAuth } from '../../../shared/api/client'
import type { SaleSummary } from '../api/posSalesApi'
import { CheckoutDialog } from './CheckoutDialog'

const sale: SaleSummary = {
  id: 'sale-1',
  organizationId: 'org-1',
  branchId: 'branch-1',
  cashRegisterId: 'register-1',
  cashShiftId: 'shift-1',
  status: 'OPEN',
  totalAmount: '100',
  version: 4,
  createdAt: '2026-10-11T12:00:00Z',
  completedAt: null,
  cancelledAt: null,
}

function response(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

function setup(options: {
  total?: string
  activeItems?: number
  operationalParentsActive?: boolean
  error?: { status: number; code: string }
  delayed?: Promise<void>
} = {}) {
  const checkoutRequests: Array<{ path: string; body: Record<string, unknown> }> = []
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = new URL(String(input), 'http://localhost').pathname
    if (path.endsWith('/sales/sale-1/checkout') && init?.method === 'POST') {
      checkoutRequests.push({
        path,
        body: JSON.parse(String(init.body)) as Record<string, unknown>,
      })
      if (options.delayed) await options.delayed
      if (options.error) return response({ code: options.error.code, message: 'Rejected' }, options.error.status)
      return response({
        sale: { ...sale, totalAmount: options.total ?? sale.totalAmount, status: 'COMPLETED', version: sale.version + 1 },
        items: [],
      })
    }
    return response({ code: 'NOT_FOUND', message: 'Not found' }, 404)
  })
  vi.stubGlobal('fetch', fetchMock)
  configureApiAuth({
    getAccessToken: () => 'test-access',
    refreshAccessToken: async () => 'test-access',
  })
  const completed = vi.fn()
  const reconciled = vi.fn()
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  const view = render(
    <QueryClientProvider client={queryClient}>
      <CheckoutDialog
        organizationId="org-1"
        branchId="branch-1"
        registerId="register-1"
        shiftId="shift-1"
        sale={{ ...sale, totalAmount: options.total ?? sale.totalAmount }}
        activeItemCount={options.activeItems ?? 1}
        operationalParentsActive={options.operationalParentsActive ?? true}
        locale="en-US"
        currency="USD"
        onClose={vi.fn()}
        onCompleted={completed}
        onReconcile={reconciled}
      />
    </QueryClientProvider>,
  )
  return { ...view, checkoutRequests, fetchMock, completed, reconciled }
}

async function chooseMethod(row: number, method: 'Efectivo' | 'Tarjeta') {
  const selectors = screen.getAllByRole('combobox', { name: 'Método de pago' })
  fireEvent.mouseDown(selectors[row])
  const listbox = await screen.findByRole('listbox')
  fireEvent.click(within(listbox).getByRole('option', { name: method }))
}

afterEach(() => {
  cleanup()
  configureApiAuth(null)
  vi.unstubAllGlobals()
})

describe('atomic checkout dialog', () => {
  it('records a full CARD payment with the current Sale version and no card or payment-history data', async () => {
    const checkout = setup()
    await chooseMethod(0, 'Tarjeta')
    fireEvent.change(screen.getByRole('textbox', { name: 'Monto aplicado' }), { target: { value: '100.0000' } })
    expect(screen.getByRole('button', { name: 'Completar venta' }).hasAttribute('disabled')).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: 'Completar venta' }))

    await waitFor(() => expect(checkout.checkoutRequests).toHaveLength(1))
    expect(checkout.checkoutRequests[0]).toEqual({
      path: '/api/v1/organizations/org-1/branches/branch-1/cash-registers/register-1/cash-shifts/shift-1/sales/sale-1/checkout',
      body: {
        expectedSaleVersion: 4,
        payments: [{ method: 'CARD', amount: '100' }],
      },
    })
    expect(checkout.fetchMock.mock.calls.some(([input]) => String(input).endsWith('/payments'))).toBe(false)
    expect(checkout.completed).toHaveBeenCalledTimes(1)
    expect(checkout.completed.mock.calls[0][0]).toEqual(expect.objectContaining({
      sale: expect.objectContaining({ status: 'COMPLETED' }),
    }))
  })

  it('shows exact CASH change and sends tendered amount without sending change', async () => {
    const checkout = setup({ total: '125.0001' })
    await chooseMethod(0, 'Efectivo')
    fireEvent.change(screen.getByRole('textbox', { name: 'Monto aplicado' }), { target: { value: '125.0001' } })
    fireEvent.change(screen.getByRole('textbox', { name: 'Efectivo recibido' }), { target: { value: '200.0000' } })

    expect(screen.getByText('Cambio estimado: $74.9999')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Completar venta' }))
    await waitFor(() => expect(checkout.checkoutRequests).toHaveLength(1))
    expect(checkout.checkoutRequests[0].body).toEqual({
      expectedSaleVersion: 4,
      payments: [{ method: 'CASH', amount: '125.0001', tenderedAmount: '200' }],
    })
    expect(JSON.stringify(checkout.checkoutRequests[0].body)).not.toContain('changeAmount')
  })

  it('supports split tender with multiple CARD instructions and one CASH instruction', async () => {
    const checkout = setup()
    await chooseMethod(0, 'Tarjeta')
    fireEvent.change(screen.getByRole('textbox', { name: 'Monto aplicado' }), { target: { value: '30' } })
    fireEvent.click(screen.getByRole('button', { name: 'Agregar método de pago' }))
    await chooseMethod(1, 'Tarjeta')
    fireEvent.change(screen.getAllByRole('textbox', { name: 'Monto aplicado' })[1], { target: { value: '10' } })
    fireEvent.click(screen.getByRole('button', { name: 'Agregar método de pago' }))
    await chooseMethod(2, 'Efectivo')
    fireEvent.change(screen.getAllByRole('textbox', { name: 'Monto aplicado' })[2], { target: { value: '60' } })
    fireEvent.change(screen.getByRole('textbox', { name: 'Efectivo recibido' }), { target: { value: '100' } })

    expect(screen.getByLabelText('Restante por cubrir').textContent).toContain('$0.00')
    expect(screen.getByText('Cambio estimado: $40.00')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Completar venta' }))
    await waitFor(() => expect(checkout.checkoutRequests).toHaveLength(1))
    expect(checkout.checkoutRequests[0].body).toEqual({
      expectedSaleVersion: 4,
      payments: [
        { method: 'CARD', amount: '30' },
        { method: 'CARD', amount: '10' },
        { method: 'CASH', amount: '60', tenderedAmount: '100' },
      ],
    })
  })

  it('fills an edited payment row with the amount remaining after all other rows', async () => {
    setup()
    await chooseMethod(0, 'Tarjeta')
    fireEvent.change(screen.getAllByRole('textbox', { name: 'Monto aplicado' })[0], { target: { value: '30' } })
    fireEvent.click(screen.getByRole('button', { name: 'Agregar método de pago' }))
    await chooseMethod(1, 'Tarjeta')
    fireEvent.change(screen.getAllByRole('textbox', { name: 'Monto aplicado' })[1], { target: { value: '20' } })

    fireEvent.click(screen.getAllByRole('button', { name: /Usar restante/ })[1])
    expect((screen.getAllByRole('textbox', { name: 'Monto aplicado' })[1] as HTMLInputElement).value).toBe('70')
    expect(screen.getByLabelText('Restante por cubrir').textContent).toContain('$0.00')
  })

  it('requires exact coverage and rejects cash under-tender without submitting', async () => {
    const checkout = setup()
    await chooseMethod(0, 'Efectivo')
    fireEvent.change(screen.getByRole('textbox', { name: 'Monto aplicado' }), { target: { value: '99.9999' } })
    fireEvent.change(screen.getByRole('textbox', { name: 'Efectivo recibido' }), { target: { value: '50' } })
    expect(screen.getByText('El efectivo recibido no puede ser menor al monto aplicado.')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Completar venta' }).hasAttribute('disabled')).toBe(true)
    expect(checkout.checkoutRequests).toHaveLength(0)
  })

  it('offers no payment rows for a zero-total Sale and completes with payments empty', async () => {
    const checkout = setup({ total: '0.0000' })
    expect(screen.queryByRole('combobox', { name: 'Método de pago' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Completar venta' }).hasAttribute('disabled')).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: 'Completar venta' }))
    await waitFor(() => expect(checkout.checkoutRequests).toHaveLength(1))
    expect(checkout.checkoutRequests[0].body).toEqual({ expectedSaleVersion: 4, payments: [] })
  })

  it('blocks empty Sales, inactive parents, overpayment and an eleventh instruction', async () => {
    const empty = setup({ activeItems: 0 })
    expect(screen.getByText('Agrega al menos un producto antes de cobrar.')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Completar venta' }).hasAttribute('disabled')).toBe(true)
    empty.unmount()
    cleanup()

    const inactive = setup({ operationalParentsActive: false })
    expect(screen.getByText(/ya no permite completar el cobro/)).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Completar venta' }).hasAttribute('disabled')).toBe(true)
    inactive.unmount()
    cleanup()

    const overpaid = setup()
    await chooseMethod(0, 'Tarjeta')
    fireEvent.change(screen.getByRole('textbox', { name: 'Monto aplicado' }), { target: { value: '100.0001' } })
    expect(screen.getByText('El monto aplicado excede el total.')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Completar venta' }).hasAttribute('disabled')).toBe(true)
    overpaid.unmount()
    cleanup()

    const limited = setup()
    for (let index = 0; index < 9; index += 1) {
      fireEvent.click(screen.getByRole('button', { name: 'Agregar método de pago' }))
    }
    expect(screen.getAllByRole('combobox', { name: 'Método de pago' })).toHaveLength(10)
    expect(screen.getByRole('button', { name: 'Máximo de 10 métodos' }).hasAttribute('disabled')).toBe(true)
    limited.unmount()
  })

  it('allows only one CASH row while retaining multiple CARD rows', async () => {
    setup()
    await chooseMethod(0, 'Efectivo')
    fireEvent.click(screen.getByRole('button', { name: 'Agregar método de pago' }))
    const selectors = screen.getAllByRole('combobox', { name: 'Método de pago' })
    fireEvent.mouseDown(selectors[1])
    const listbox = await screen.findByRole('listbox')
    expect(within(listbox).queryByRole('option', { name: 'Efectivo' })).toBeNull()
    expect(within(listbox).getByRole('option', { name: 'Tarjeta' })).toBeTruthy()
  })

  it('prevents duplicate submit and only posts once while pending', async () => {
    let resolveCheckout: (() => void) | undefined
    const checkout = setup({ delayed: new Promise<void>((resolve) => { resolveCheckout = resolve }) })
    await chooseMethod(0, 'Tarjeta')
    fireEvent.change(screen.getByRole('textbox', { name: 'Monto aplicado' }), { target: { value: '100' } })
    fireEvent.click(screen.getByRole('button', { name: 'Completar venta' }))
    await screen.findByRole('button', { name: 'Procesando cobro…' })
    fireEvent.click(screen.getByRole('button', { name: 'Procesando cobro…' }))
    expect(checkout.checkoutRequests).toHaveLength(1)
    resolveCheckout?.()
    await waitFor(() => expect(checkout.completed).toHaveBeenCalledTimes(1))
  })

  it('closes and reconciles on concurrent change without retrying old instructions', async () => {
    const checkout = setup({ error: { status: 409, code: 'CONCURRENT_MODIFICATION' } })
    await chooseMethod(0, 'Tarjeta')
    fireEvent.change(screen.getByRole('textbox', { name: 'Monto aplicado' }), { target: { value: '100' } })
    fireEvent.click(screen.getByRole('button', { name: 'Completar venta' }))
    await waitFor(() => expect(checkout.reconciled).toHaveBeenCalledWith(
      'sale-1',
      'La venta cambió. Revisa el total actualizado antes de volver a cobrar.',
      false,
      false,
    ))
    expect(checkout.checkoutRequests).toHaveLength(1)
  })
})
