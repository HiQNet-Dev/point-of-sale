import { requestJson } from '../../../shared/api/client'

export type InventoryNegativeStockPolicy = 'STRICT' | 'ALLOW_NEGATIVE'

/** Organization response shape used by the inventory settings screen. */
export interface Organization {
  id: string
  name: string
  code: string
  timezone: string
  currency: string
  locale: string
  status: string
  inventoryNegativeStockPolicy: InventoryNegativeStockPolicy
  version: number
  createdAt: string
  updatedAt: string
}

function segment(value: string): string {
  return encodeURIComponent(value)
}

export function getOrganization(organizationId: string): Promise<Organization> {
  return requestJson(`/organizations/${segment(organizationId)}`)
}

export interface UpdateOrganizationPolicyInput {
  expectedVersion: number
  inventoryNegativeStockPolicy: InventoryNegativeStockPolicy
}

/**
 * Partial Organization update: only the negative-stock policy and the version
 * guard are sent, so name/code/timezone/currency/locale/status are never
 * overwritten with stale values.
 */
export function updateOrganizationPolicy(
  organizationId: string,
  input: UpdateOrganizationPolicyInput,
): Promise<Organization> {
  return requestJson(`/organizations/${segment(organizationId)}`, {
    method: 'PATCH',
    body: JSON.stringify(input),
  })
}
