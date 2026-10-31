import type { commands } from './purchasing.generated'
import { DesktopCommandError, unwrap } from './desktop-client'

/** Firmas derivadas del registro Rust preparado, no tipos de dominio manuales. */
export type PurchasingTransport = typeof commands
type Request<K extends keyof PurchasingTransport> = Parameters<PurchasingTransport[K]>[0]

/**
 * Servicio único de Purchasing. Por defecto falla cerrado SIN invocar Tauri.
 * La composición autorizada futura inyectará el transporte generado; las
 * pantallas comerciales actuales nunca se conectan a fixtures por esta vía.
 */
export function createPurchasingService(transport?: PurchasingTransport) {
  function activeTransport(): PurchasingTransport {
    if (!transport) throw new DesktopCommandError('OPERATION_NOT_AVAILABLE')
    return transport
  }
  return {
    available: transport !== undefined,
    createSupplier: async (request: Request<'purchasingCreateSupplier'>) => unwrap(activeTransport().purchasingCreateSupplier(request)),
    getSupplier: async (request: Request<'purchasingGetSupplier'>) => unwrap(activeTransport().purchasingGetSupplier(request)),
    listSuppliers: async (request: Request<'purchasingListSuppliers'>) => unwrap(activeTransport().purchasingListSuppliers(request)),
    updateSupplier: async (request: Request<'purchasingUpdateSupplier'>) => unwrap(activeTransport().purchasingUpdateSupplier(request)),
    createOrder: async (request: Request<'purchasingCreateOrder'>) => unwrap(activeTransport().purchasingCreateOrder(request)),
    getOrder: async (request: Request<'purchasingGetOrder'>) => unwrap(activeTransport().purchasingGetOrder(request)),
    listOrders: async (request: Request<'purchasingListOrders'>) => unwrap(activeTransport().purchasingListOrders(request)),
    changeSupplier: async (request: Request<'purchasingChangeSupplier'>) => unwrap(activeTransport().purchasingChangeSupplier(request)),
    addLine: async (request: Request<'purchasingAddLine'>) => unwrap(activeTransport().purchasingAddLine(request)),
    updateLine: async (request: Request<'purchasingUpdateLine'>) => unwrap(activeTransport().purchasingUpdateLine(request)),
    removeLine: async (request: Request<'purchasingRemoveLine'>) => unwrap(activeTransport().purchasingRemoveLine(request)),
    issueOrder: async (request: Request<'purchasingIssueOrder'>) => unwrap(activeTransport().purchasingIssueOrder(request)),
    cancelOrder: async (request: Request<'purchasingCancelOrder'>) => unwrap(activeTransport().purchasingCancelOrder(request)),
    postReceipt: async (request: Request<'purchasingPostReceipt'>) => unwrap(activeTransport().purchasingPostReceipt(request)),
    getReceipt: async (request: Request<'purchasingGetReceipt'>) => unwrap(activeTransport().purchasingGetReceipt(request)),
    listReceipts: async (request: Request<'purchasingListReceipts'>) => unwrap(activeTransport().purchasingListReceipts(request)),
    receiptStatus: async (request: Request<'purchasingReceiptStatus'>) => unwrap(activeTransport().purchasingReceiptStatus(request)),
  }
}

/** Sin transporte hasta aprobar identidad y persistencia; no es un cliente lab. */
export const purchasingService = createPurchasingService()
export type PurchasingService = ReturnType<typeof createPurchasingService>
