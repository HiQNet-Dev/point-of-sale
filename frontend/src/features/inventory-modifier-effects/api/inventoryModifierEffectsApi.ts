import { requestJson } from '../../../shared/api/client'
import { pagingParams, segment } from '../../../shared/api/pagination'
import type { ListQuery, PageResponse } from '../../../shared/api/pagination'
import type { InventoryBaseUnit, InventoryItemStatus } from '../../inventory-items/api/inventoryItemsApi'

export const MODIFIER_STATUS_LABELS: Record<'ACTIVE' | 'INACTIVE', string> = {
  ACTIVE: 'Activo',
  INACTIVE: 'Inactivo',
}

export interface ModifierGroup {
  id: string
  organizationId: string
  code: string
  name: string
  status: 'ACTIVE' | 'INACTIVE'
  version: number
}

export interface ModifierOption {
  id: string
  organizationId: string
  modifierGroupId: string
  code: string
  name: string
  displayOrder: number
  status: 'ACTIVE' | 'INACTIVE'
  version: number
}

export interface ModifierGroupListQuery extends ListQuery {
  status?: 'ACTIVE' | 'INACTIVE'
}

export const MODIFIER_GROUPS_PAGE_SIZE = 100
export const MODIFIER_OPTIONS_PAGE_SIZE = 100

export function listModifierGroups(
  organizationId: string,
  query: ModifierGroupListQuery,
): Promise<PageResponse<ModifierGroup>> {
  const params = pagingParams(query, { status: query.status })
  return requestJson(`/organizations/${segment(organizationId)}/modifier-groups?${params}`)
}

export function listModifierOptions(
  organizationId: string,
  modifierGroupId: string,
  query: ListQuery,
): Promise<PageResponse<ModifierOption>> {
  const params = pagingParams(query)
  return requestJson(
    `/organizations/${segment(organizationId)}/modifier-groups/${segment(modifierGroupId)}/options?${params}`,
  )
}

export interface ModifierEffect {
  inventoryItemId: string
  code: string
  name: string
  baseUnit: InventoryBaseUnit
  fractionalUnitAllowed: boolean
  status: InventoryItemStatus
  quantityDelta: string
}

export interface ModifierEffects {
  id: string
  organizationId: string
  modifierOptionId: string
  version: number
  createdAt: string
  updatedAt: string
  effects: ModifierEffect[]
}

export interface ModifierEffectInput {
  inventoryItemId: string
  quantityDelta: string
}

export interface ReplaceModifierEffectsInput {
  expectedVersion: number
  effects: ModifierEffectInput[]
}

function effectsPath(organizationId: string, modifierOptionId: string): string {
  return `/organizations/${segment(organizationId)}/modifier-options/${segment(modifierOptionId)}/inventory-effects`
}

export function getModifierEffects(organizationId: string, modifierOptionId: string): Promise<ModifierEffects> {
  return requestJson(effectsPath(organizationId, modifierOptionId))
}

export function replaceModifierEffects(
  organizationId: string,
  modifierOptionId: string,
  input: ReplaceModifierEffectsInput,
): Promise<ModifierEffects> {
  return requestJson(effectsPath(organizationId, modifierOptionId), {
    method: 'PUT',
    body: JSON.stringify(input),
  })
}
