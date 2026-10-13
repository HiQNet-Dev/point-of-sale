import type { InventoryNegativeStockPolicy } from '../api/organizationApi'

export const INVENTORY_NEGATIVE_STOCK_POLICIES: readonly InventoryNegativeStockPolicy[] = [
  'STRICT',
  'ALLOW_NEGATIVE',
]

export const INVENTORY_NEGATIVE_STOCK_POLICY_LABELS: Record<InventoryNegativeStockPolicy, string> = {
  STRICT: 'No permitir stock negativo',
  ALLOW_NEGATIVE: 'Permitir stock negativo',
}

export const INVENTORY_NEGATIVE_STOCK_POLICY_DESCRIPTIONS: Record<InventoryNegativeStockPolicy, string> = {
  STRICT:
    'Impide completar una venta cuando el inventario rastreado no tiene existencia suficiente.',
  ALLOW_NEGATIVE:
    'Permite completar la venta aunque el inventario rastreado quede por debajo de cero.',
}
