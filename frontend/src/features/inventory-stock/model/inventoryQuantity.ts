/**
 * Exact decimal-string handling for inventory quantities. Quantities cross the
 * API boundary as decimal strings (NUMERIC(19,6)); binary floating point is
 * never used for parsing, comparing or previewing stock.
 */
const QUANTITY = /^-?\d+(?:\.\d{1,6})?$/

/** Validates an opening quantity: a non-negative decimal string with <=6 decimals. */
export function validateOpeningQuantity(value: string): string | null {
  const text = value.trim()
  if (!text) return 'Ingresa una cantidad.'
  if (!QUANTITY.test(text)) return 'Usa un número no negativo con hasta 6 decimales.'
  if (text.startsWith('-')) return 'La cantidad de apertura no puede ser negativa.'
  return null
}

/** Validates an adjustment delta: a signed, non-zero decimal string with <=6 decimals. */
export function validateAdjustmentDelta(value: string): string | null {
  const text = value.trim()
  if (!text) return 'Ingresa un ajuste.'
  if (!QUANTITY.test(text)) return 'Usa un número (puede ser negativo) con hasta 6 decimales.'
  if (parseScaledQuantity(text) === 0n) return 'El ajuste no puede ser cero.'
  return null
}

/** Trims insignificant trailing zeros for display, e.g. "975.000000" -> "975". */
export function formatQuantity(value: string): string {
  if (!value.includes('.')) return value
  const trimmed = value.replace(/0+$/, '').replace(/\.$/, '')
  if (trimmed === '' || trimmed === '-' || trimmed === '-0') return '0'
  return trimmed
}

/** Parses a decimal string into an integer scaled by 1e6, or null if invalid. */
export function parseScaledQuantity(value: string): bigint | null {
  const text = value.trim()
  if (!QUANTITY.test(text)) return null
  const negative = text.startsWith('-')
  const body = negative ? text.slice(1) : text
  const [integerPart, fractionPart = ''] = body.split('.')
  const fraction = (fractionPart + '000000').slice(0, 6)
  const scaled = BigInt(integerPart) * 1_000_000n + BigInt(fraction)
  return negative ? -scaled : scaled
}

/** Renders an integer scaled by 1e6 back into a trimmed decimal string. */
export function formatScaledQuantity(scaled: bigint): string {
  const negative = scaled < 0n
  const absolute = negative ? -scaled : scaled
  const integerPart = absolute / 1_000_000n
  const fraction = (absolute % 1_000_000n).toString().padStart(6, '0').replace(/0+$/, '')
  const body = fraction ? `${integerPart}.${fraction}` : `${integerPart}`
  return negative ? `-${body}` : body
}
