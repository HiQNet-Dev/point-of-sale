import { describe, expect, it } from 'vitest'
import {
  formatQuantity,
  formatScaledQuantity,
  parseScaledQuantity,
  validateAdjustmentDelta,
  validateOpeningQuantity,
} from './inventoryQuantity'

describe('inventory quantity strings', () => {
  it('validates opening quantities as non-negative decimals with up to 6 places', () => {
    expect(validateOpeningQuantity('')).toBe('Ingresa una cantidad.')
    expect(validateOpeningQuantity('abc')).toBe('Usa un número no negativo con hasta 6 decimales.')
    expect(validateOpeningQuantity('-1')).toBe('La cantidad de apertura no puede ser negativa.')
    expect(validateOpeningQuantity('1.1234567')).toBe('Usa un número no negativo con hasta 6 decimales.')
    expect(validateOpeningQuantity('0')).toBeNull()
    expect(validateOpeningQuantity('10')).toBeNull()
    expect(validateOpeningQuantity('10.5')).toBeNull()
    expect(validateOpeningQuantity('0.000001')).toBeNull()
  })

  it('validates adjustment deltas as signed, non-zero decimals', () => {
    expect(validateAdjustmentDelta('')).toBe('Ingresa un ajuste.')
    expect(validateAdjustmentDelta('x')).toBe('Usa un número (puede ser negativo) con hasta 6 decimales.')
    expect(validateAdjustmentDelta('0')).toBe('El ajuste no puede ser cero.')
    expect(validateAdjustmentDelta('-0')).toBe('El ajuste no puede ser cero.')
    expect(validateAdjustmentDelta('2')).toBeNull()
    expect(validateAdjustmentDelta('-2.5')).toBeNull()
  })

  it('trims insignificant trailing zeros for display without float rounding', () => {
    expect(formatQuantity('975.000000')).toBe('975')
    expect(formatQuantity('1.500000')).toBe('1.5')
    expect(formatQuantity('0.000000')).toBe('0')
    expect(formatQuantity('-0.000000')).toBe('0')
    expect(formatQuantity('-2.500000')).toBe('-2.5')
    expect(formatQuantity('1000')).toBe('1000')
  })

  it('parses and formats exact scaled values round-trip', () => {
    expect(parseScaledQuantity('1')).toBe(1_000_000n)
    expect(parseScaledQuantity('1.5')).toBe(1_500_000n)
    expect(parseScaledQuantity('-2.25')).toBe(-2_250_000n)
    expect(parseScaledQuantity('1.123456')).toBe(1_123_456n)
    expect(parseScaledQuantity('abc')).toBeNull()
    expect(parseScaledQuantity('1.')).toBeNull()

    expect(formatScaledQuantity(1_500_000n)).toBe('1.5')
    expect(formatScaledQuantity(-2_250_000n)).toBe('-2.25')
    expect(formatScaledQuantity(0n)).toBe('0')
    expect(formatScaledQuantity(1_000_000n)).toBe('1')
  })
})
