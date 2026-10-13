import { parseScaledQuantity } from '../../inventory-stock/model/inventoryQuantity'
import type { InventoryBaseUnit } from '../../inventory-items/api/inventoryItemsApi'

/**
 * Exact decimal-string validation shared by recipe quantities and modifier
 * effect deltas. Reuses the UI-A scaled-parsing helper so no divergent decimal
 * logic (and no floating point) is introduced.
 */
const SCALED_UNIT = 1_000_000n

export interface QuantityUnitRule {
  baseUnit: InventoryBaseUnit
  fractionalUnitAllowed: boolean
}

function discreteFractionError(rule: QuantityUnitRule | null, value: string): string | null {
  if (!rule || rule.baseUnit !== 'UNIT' || rule.fractionalUnitAllowed) return null
  const scaled = parseScaledQuantity(value)
  if (scaled === null || scaled % SCALED_UNIT === 0n) return null
  return 'Este artículo no admite fracciones; usa una cantidad entera.'
}

/** Positive decimal string (> 0, <= 6 decimals) for a recipe component. */
export function validateRecipeQuantity(value: string, rule: QuantityUnitRule | null): string | null {
  const text = value.trim()
  if (!text) return 'Ingresa una cantidad.'
  const scaled = parseScaledQuantity(text)
  if (scaled === null) return 'Usa un número con hasta 6 decimales.'
  if (scaled <= 0n) return 'La cantidad debe ser mayor que cero.'
  return discreteFractionError(rule, text)
}

/** Signed, non-zero decimal string for a modifier inventory effect delta. */
export function validateEffectDelta(value: string, rule: QuantityUnitRule | null): string | null {
  const text = value.trim()
  if (!text) return 'Ingresa un valor.'
  const scaled = parseScaledQuantity(text)
  if (scaled === null) return 'Usa un número (puede ser negativo) con hasta 6 decimales.'
  if (scaled === 0n) return 'El valor no puede ser cero.'
  return discreteFractionError(rule, text)
}
