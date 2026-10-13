import { requestJson } from '../../../shared/api/client'

export type InventoryNegativeStockPolicy = 'STRICT' | 'ALLOW_NEGATIVE'

/** Forma de la respuesta Organization usada por la pantalla de configuración de inventario. */
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
 * Actualización parcial de Organization: solo se envían la política de stock
 * negativo y el guard de versión, así que name/code/timezone/currency/locale/status
 * nunca se sobrescriben con valores stale.
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
