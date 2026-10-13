import type { InventoryItemListQuery } from '../api/inventoryItemsApi'

export const inventoryItemsKeys = {
  all: ['inventory', 'items'] as const,
  list: (organizationId: string, query: InventoryItemListQuery) =>
    [...inventoryItemsKeys.all, 'list', organizationId, query] as const,
}
