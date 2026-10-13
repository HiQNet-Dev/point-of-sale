// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { BranchStock } from '../api/inventoryStockApi'
import { StockMutationDialog } from './StockMutationDialog'
import type { StockMutationMode } from './StockMutationDialog'

const STOCK: BranchStock = {
  inventoryItemId: 'item-1',
  code: 'harina',
  name: 'Harina',
  baseUnit: 'UNIT',
  fractionalUnitAllowed: true,
  status: 'ACTIVE',
  quantityOnHand: '10.000000',
  version: 0,
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

function mutationResult() {
  return {
    inventoryItemId: 'item-1',
    movementId: 'mov-1',
    type: 'ADJUSTMENT',
    quantityDelta: '0.000000',
    quantityOnHand: '10.000000',
    version: 1,
  }
}

function renderDialog(mode: StockMutationMode, fetchMock: ReturnType<typeof vi.fn>) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  vi.stubGlobal('fetch', fetchMock)
  const onClose = vi.fn()
  const onSaved = vi.fn()
  render(
    <QueryClientProvider client={queryClient}>
      <StockMutationDialog
        open
        mode={mode}
        organizationId="org-1"
        branchId="b-1"
        stock={STOCK}
        onClose={onClose}
        onSaved={onSaved}
      />
    </QueryClientProvider>,
  )
  return { onClose, onSaved }
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('StockMutationDialog', () => {
  it('records an opening with an exact decimal string and omits an empty reason', async () => {
    const fetchMock = vi.fn(async () => json(mutationResult()))
    const { onClose, onSaved } = renderDialog('opening', fetchMock)

    fireEvent.change(screen.getByLabelText(/Cantidad de apertura/), { target: { value: '10.500000' } })
    fireEvent.click(screen.getByRole('button', { name: 'Registrar apertura' }))

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1))
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit]
    expect(String(url)).toContain('/organizations/org-1/branches/b-1/inventory-items/item-1/opening-stock')
    expect(JSON.parse(String(init.body))).toEqual({ expectedVersion: 0, quantity: '10.500000' })
    expect(onSaved).toHaveBeenCalled()
    expect(onClose).toHaveBeenCalled()
  })

  it('rejects a negative opening quantity before calling the backend', () => {
    const fetchMock = vi.fn()
    renderDialog('opening', fetchMock)

    fireEvent.change(screen.getByLabelText(/Cantidad de apertura/), { target: { value: '-1' } })
    fireEvent.click(screen.getByRole('button', { name: 'Registrar apertura' }))

    expect(screen.getByText('La cantidad de apertura no puede ser negativa.')).toBeTruthy()
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('previews the projected balance for an adjustment without float rounding', () => {
    renderDialog('adjustment', vi.fn())
    fireEvent.change(screen.getByLabelText(/Ajuste \(\+\/-\)/), { target: { value: '-2.5' } })
    expect(screen.getByText('Existencia resultante estimada: 7.5')).toBeTruthy()
  })

  it('requires a reason for an adjustment', async () => {
    const fetchMock = vi.fn(async () => json(mutationResult()))
    renderDialog('adjustment', fetchMock)

    fireEvent.change(screen.getByLabelText(/Ajuste \(\+\/-\)/), { target: { value: '-2.5' } })
    fireEvent.click(screen.getByRole('button', { name: 'Registrar ajuste' }))

    expect(screen.getByText('Ingresa un motivo para el ajuste.')).toBeTruthy()
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('sends the adjustment delta and reason as strings', async () => {
    const fetchMock = vi.fn(async () => json(mutationResult()))
    renderDialog('adjustment', fetchMock)

    fireEvent.change(screen.getByLabelText(/Ajuste \(\+\/-\)/), { target: { value: '-2.5' } })
    fireEvent.change(screen.getByLabelText(/Motivo/), { target: { value: 'Merma' } })
    fireEvent.click(screen.getByRole('button', { name: 'Registrar ajuste' }))

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1))
    const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit]
    expect(JSON.parse(String(init.body))).toEqual({ expectedVersion: 0, quantityDelta: '-2.5', reason: 'Merma' })
  })

  it('surfaces a concurrent modification and refreshes without closing', async () => {
    const fetchMock = vi.fn(async () => json({ code: 'CONCURRENT_MODIFICATION', message: 'Changed' }, 409))
    const { onClose, onSaved } = renderDialog('adjustment', fetchMock)

    fireEvent.change(screen.getByLabelText(/Ajuste \(\+\/-\)/), { target: { value: '-2.5' } })
    fireEvent.change(screen.getByLabelText(/Motivo/), { target: { value: 'Merma' } })
    fireEvent.click(screen.getByRole('button', { name: 'Registrar ajuste' }))

    expect(await screen.findByText('Las existencias cambiaron desde que se cargaron. Se actualizarán los datos.')).toBeTruthy()
    expect(onSaved).toHaveBeenCalled()
    expect(onClose).not.toHaveBeenCalled()
  })
})
