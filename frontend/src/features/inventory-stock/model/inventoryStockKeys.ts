import type { BranchStockListQuery } from '../api/inventoryStockApi'

export const inventoryStockKeys = {
  all: ['inventory', 'stock'] as const,
  list: (organizationId: string, branchId: string, query: BranchStockListQuery) =>
    [...inventoryStockKeys.all, 'list', organizationId, branchId, query] as const,
  detail: (organizationId: string, branchId: string, inventoryItemId: string) =>
    [...inventoryStockKeys.all, 'detail', organizationId, branchId, inventoryItemId] as const,
  movements: (organizationId: string, branchId: string, inventoryItemId: string, page: number) =>
    [...inventoryStockKeys.all, 'movements', organizationId, branchId, inventoryItemId, page] as const,
}
