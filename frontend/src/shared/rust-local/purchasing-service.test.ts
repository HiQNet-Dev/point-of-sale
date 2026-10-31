import { beforeEach, describe, expect, it, vi } from 'vitest'
import { QueryClient } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import { commands, type OrderDto_Serialize } from './purchasing.generated'
import { createPurchasingService, purchasingService } from './purchasing-service'
import { purchasingQueries } from './purchasing-queries'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
const mockedInvoke = vi.mocked(invoke)

const order: OrderDto_Serialize = {
  id: 'order-id', branchId: 'branch-id', supplierId: 'supplier-id', code: 'oc-2c', status: 'DRAFT',
  currency: 'MXN', supplierCode: 'supplier', supplierName: 'Proveedor',
  version: '9007199254740993', total: null,
  lines: [{ id: 'line-id', lineNumber: 1, inventoryItemId: 'item-id', itemCode: 'CAFE', itemName: 'Café', unit: 'UNIT', quantity: '2.000000', unitCost: null, lineTotal: null, receivedQuantity: '0.000000', fullyReceived: false }],
}

beforeEach(() => { mockedInvoke.mockReset() })

describe('Purchasing: frontera preparada, no activación comercial', () => {
  it('falla cerrado por defecto y no invoca IPC', async () => {
    expect(purchasingService.available).toBe(false)
    await expect(purchasingService.listSuppliers({ limit: 20, offset: 0 })).rejects.toMatchObject({ code: 'OPERATION_NOT_AVAILABLE' })
    expect(mockedInvoke).not.toHaveBeenCalled()
    expect(purchasingQueries(purchasingService, 'cache-epoch-1').suppliers({ limit: 20, offset: 0 }).enabled).toBe(false)
  })

  it('usa el comando generado y conserva costo desconocido y revisión mayor que MAX_SAFE_INTEGER', async () => {
    mockedInvoke.mockResolvedValue(order)
    const service = createPurchasingService(commands)
    const response = await service.getOrder({ branchId: 'branch-id', orderId: 'order-id' })
    expect(mockedInvoke).toHaveBeenCalledWith('purchasing_get_order', { request: { branchId: 'branch-id', orderId: 'order-id' } })
    expect(response.version).toBe('9007199254740993')
    expect(response.total).toBeNull()
    expect(response.lines[0].unitCost).toBeNull()
  })

  it('envía cero conocido como texto y distingue el patch clear de keep', async () => {
    mockedInvoke.mockResolvedValue(order)
    const service = createPurchasingService(commands)
    const base = { branchId: 'branch-id', orderId: 'order-id', lineId: 'line-id', expectedVersion: order.version, quantity: '2.000000' }
    await service.updateLine({ ...base, unitCost: { operation: 'set', value: '0.0000' } })
    expect(mockedInvoke).toHaveBeenLastCalledWith('purchasing_update_line', { request: { ...base, unitCost: { operation: 'set', value: '0.0000' } } })
    await service.updateLine({ ...base, unitCost: { operation: 'clear' } })
    expect(mockedInvoke).toHaveBeenLastCalledWith('purchasing_update_line', { request: { ...base, unitCost: { operation: 'clear' } } })
  })

  it('propaga conflictos tipados sin reintentar una mutación', async () => {
    mockedInvoke.mockRejectedValue({ code: 'CONFLICT' })
    await expect(createPurchasingService(commands).issueOrder({ branchId: 'branch-id', orderId: 'order-id', expectedVersion: '1' })).rejects.toMatchObject({ code: 'CONFLICT' })
    expect(mockedInvoke).toHaveBeenCalledTimes(1)
  })

  it('oculta los detalles de errores de transporte', async () => {
    mockedInvoke.mockRejectedValue(new Error('detalle interno de almacenamiento'))
    await expect(createPurchasingService(commands).getSupplier({ supplierId: 'supplier-id' })).rejects.toMatchObject({ message: 'INTERNAL_ERROR', code: 'INTERNAL_ERROR' })
  })

  it('integra QueryClient con claves segregadas por sesión y sucursal', async () => {
    mockedInvoke.mockResolvedValue([order])
    const service = createPurchasingService(commands)
    const client = new QueryClient()
    const request = { branchId: 'branch-id', limit: 20, offset: 0 }
    const options = purchasingQueries(service, 'cache-epoch-1').orders(request)
    expect(options.enabled).toBe(true)
    expect(options.retry).toBe(false)
    expect(await client.fetchQuery(options)).toEqual([order])
    expect(options.queryKey).not.toEqual(purchasingQueries(service, 'cache-epoch-2').orders(request).queryKey)
    expect(options.queryKey).not.toEqual(purchasingQueries(service, 'cache-epoch-1').orders({ ...request, branchId: 'other' }).queryKey)
    expect(purchasingQueries(service, '').orders(request).enabled).toBe(false)
    client.clear()
  })
})
