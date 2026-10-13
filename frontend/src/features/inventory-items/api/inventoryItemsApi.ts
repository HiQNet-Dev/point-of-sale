import { requestJson } from '../../../shared/api/client'
import { pagingParams, segment } from '../../../shared/api/pagination'
import type { PageResponse } from '../../../shared/api/pagination'

export const INVENTORY_BASE_UNITS = ['UNIT', 'GRAM', 'MILLILITER'] as const
export type InventoryBaseUnit = (typeof INVENTORY_BASE_UNITS)[number]

export const INVENTORY_ITEM_STATUSES = ['ACTIVE', 'INACTIVE'] as const
export type InventoryItemStatus = (typeof INVENTORY_ITEM_STATUSES)[number]

export const BASE_UNIT_LABELS: Record<InventoryBaseUnit, string> = {
  UNIT: 'Unidad',
  GRAM: 'Gramo',
  MILLILITER: 'Mililitro',
}

/** Compact unit suffix used next to exact quantity inputs (no unit conversion). */
export const QUANTITY_UNIT_SYMBOLS: Record<InventoryBaseUnit, string> = {
  UNIT: 'unidad',
  GRAM: 'g',
  MILLILITER: 'ml',
}

export const ITEM_STATUS_LABELS: Record<InventoryItemStatus, string> = {
  ACTIVE: 'Activo',
  INACTIVE: 'Inactivo',
}

export interface InventoryItem {
  id: string
  organizationId: string
  code: string
  name: string
  description: string | null
  baseUnit: InventoryBaseUnit
  fractionalUnitAllowed: boolean
  status: InventoryItemStatus
  version: number
  createdAt: string
  updatedAt: string
}

export interface InventoryItemListQuery {
  page: number
  size: number
  q?: string
  status?: InventoryItemStatus
  baseUnit?: InventoryBaseUnit
}

export interface CreateInventoryItemInput {
  code: string
  name: string
  description?: string
  baseUnit: InventoryBaseUnit
  fractionalUnitAllowed: boolean
}

export interface UpdateInventoryItemInput {
  expectedVersion: number
  name?: string
  description?: string
  baseUnit?: InventoryBaseUnit
  fractionalUnitAllowed?: boolean
  status?: InventoryItemStatus
}

export const INVENTORY_ITEMS_PAGE_SIZE = 20

function basePath(organizationId: string): string {
  return `/organizations/${segment(organizationId)}/inventory-items`
}

export function listInventoryItems(
  organizationId: string,
  query: InventoryItemListQuery,
): Promise<PageResponse<InventoryItem>> {
  const params = pagingParams(query, { status: query.status, baseUnit: query.baseUnit })
  return requestJson(`${basePath(organizationId)}?${params}`)
}

export function getInventoryItem(organizationId: string, inventoryItemId: string): Promise<InventoryItem> {
  return requestJson(`${basePath(organizationId)}/${segment(inventoryItemId)}`)
}

export function createInventoryItem(
  organizationId: string,
  input: CreateInventoryItemInput,
): Promise<InventoryItem> {
  return requestJson(basePath(organizationId), { method: 'POST', body: JSON.stringify(input) })
}

export function updateInventoryItem(
  organizationId: string,
  inventoryItemId: string,
  input: UpdateInventoryItemInput,
): Promise<InventoryItem> {
  return requestJson(`${basePath(organizationId)}/${segment(inventoryItemId)}`, {
    method: 'PATCH',
    body: JSON.stringify(input),
  })
}
