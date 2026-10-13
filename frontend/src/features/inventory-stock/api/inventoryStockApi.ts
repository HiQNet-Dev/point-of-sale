import { requestJson } from '../../../shared/api/client'
import { pagingParams, segment } from '../../../shared/api/pagination'
import type { PageResponse } from '../../../shared/api/pagination'
import type { InventoryBaseUnit, InventoryItemStatus } from '../../inventory-items/api/inventoryItemsApi'

export const MOVEMENT_TYPES = ['OPENING', 'ADJUSTMENT', 'SALE_CONSUMPTION'] as const
export type InventoryMovementType = (typeof MOVEMENT_TYPES)[number]

export const MOVEMENT_TYPE_LABELS: Record<InventoryMovementType, string> = {
  OPENING: 'Apertura',
  ADJUSTMENT: 'Ajuste',
  SALE_CONSUMPTION: 'Venta',
}

export interface BranchStock {
  inventoryItemId: string
  code: string
  name: string
  baseUnit: InventoryBaseUnit
  fractionalUnitAllowed: boolean
  status: InventoryItemStatus
  quantityOnHand: string
  version: number
}

export interface InventoryMovement {
  id: string
  type: InventoryMovementType
  quantityDelta: string
  resultingQuantity: string
  reason: string | null
  actorUserId: string
  saleId: string | null
  occurredAt: string
}

export interface BranchStockListQuery {
  page: number
  size: number
  q?: string
  status?: InventoryItemStatus
  baseUnit?: InventoryBaseUnit
}

export interface StockMutationResult {
  inventoryItemId: string
  movementId: string
  type: InventoryMovementType
  quantityDelta: string
  quantityOnHand: string
  version: number
}

export interface OpeningStockInput {
  expectedVersion: number
  quantity: string
  reason?: string
}

export interface AdjustmentInput {
  expectedVersion: number
  quantityDelta: string
  reason: string
}

/** Quantity fields are exact decimal strings; never coerce them to numbers. */
const QUANTITY_FIELDS = ['quantityOnHand', 'quantityDelta', 'resultingQuantity'] as const

export const INVENTORY_STOCK_PAGE_SIZE = 20

function branchBase(organizationId: string, branchId: string): string {
  return `/organizations/${segment(organizationId)}/branches/${segment(branchId)}/inventory-items`
}

export function listBranchStock(
  organizationId: string,
  branchId: string,
  query: BranchStockListQuery,
): Promise<PageResponse<BranchStock>> {
  const params = pagingParams(query, { status: query.status, baseUnit: query.baseUnit })
  return requestJson(`${branchBase(organizationId, branchId)}?${params}`, {}, { exactDecimalFields: QUANTITY_FIELDS })
}

export function getBranchStock(
  organizationId: string,
  branchId: string,
  inventoryItemId: string,
): Promise<BranchStock> {
  return requestJson(
    `${branchBase(organizationId, branchId)}/${segment(inventoryItemId)}`,
    {},
    { exactDecimalFields: QUANTITY_FIELDS },
  )
}

export function listInventoryMovements(
  organizationId: string,
  branchId: string,
  inventoryItemId: string,
  page: number,
  size: number,
): Promise<PageResponse<InventoryMovement>> {
  const params = new URLSearchParams({ page: String(page), size: String(size) })
  return requestJson(
    `${branchBase(organizationId, branchId)}/${segment(inventoryItemId)}/movements?${params}`,
    {},
    { exactDecimalFields: QUANTITY_FIELDS },
  )
}

export function recordOpeningStock(
  organizationId: string,
  branchId: string,
  inventoryItemId: string,
  input: OpeningStockInput,
): Promise<StockMutationResult> {
  return requestJson(
    `${branchBase(organizationId, branchId)}/${segment(inventoryItemId)}/opening-stock`,
    { method: 'POST', body: JSON.stringify(input) },
    { exactDecimalFields: QUANTITY_FIELDS },
  )
}

export function recordStockAdjustment(
  organizationId: string,
  branchId: string,
  inventoryItemId: string,
  input: AdjustmentInput,
): Promise<StockMutationResult> {
  return requestJson(
    `${branchBase(organizationId, branchId)}/${segment(inventoryItemId)}/adjustments`,
    { method: 'POST', body: JSON.stringify(input) },
    { exactDecimalFields: QUANTITY_FIELDS },
  )
}
