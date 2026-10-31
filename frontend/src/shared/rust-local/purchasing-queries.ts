import { queryOptions } from '@tanstack/react-query'
import type { PurchasingService } from './purchasing-service'

/** Las claves incluyen sesión/scope para no compartir datos entre identidades. */
export function purchasingQueries(service: PurchasingService, sessionScope: string) {
  const scope = sessionScope.trim()
  const enabled = service.available && scope.length > 0
  const root = ['rust-purchasing', scope] as const
  return {
    root,
    suppliers: (request: Parameters<PurchasingService['listSuppliers']>[0]) => queryOptions({
      queryKey: [...root, 'suppliers', request],
      queryFn: () => service.listSuppliers(request),
      enabled,
      retry: false,
    }),
    orders: (request: Parameters<PurchasingService['listOrders']>[0]) => queryOptions({
      queryKey: [...root, 'orders', request],
      queryFn: () => service.listOrders(request),
      enabled,
      retry: false,
    }),
    order: (request: Parameters<PurchasingService['getOrder']>[0]) => queryOptions({
      queryKey: [...root, 'order', request],
      queryFn: () => service.getOrder(request),
      enabled,
      retry: false,
    }),
    receipts: (request: Parameters<PurchasingService['listReceipts']>[0]) => queryOptions({
      queryKey: [...root, 'receipts', request],
      queryFn: () => service.listReceipts(request),
      enabled,
      retry: false,
    }),
    receiptStatus: (request: Parameters<PurchasingService['receiptStatus']>[0]) => queryOptions({
      queryKey: [...root, 'receipt-status', request],
      queryFn: () => service.receiptStatus(request),
      enabled,
      retry: false,
    }),
  }
}
