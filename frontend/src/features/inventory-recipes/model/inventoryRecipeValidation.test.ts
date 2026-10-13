import { describe, expect, it } from 'vitest'
import { validateEffectDelta, validateRecipeQuantity } from './inventoryRecipeValidation'
import type { QuantityUnitRule } from './inventoryRecipeValidation'

const gram: QuantityUnitRule = { baseUnit: 'GRAM', fractionalUnitAllowed: true }
const discreteUnit: QuantityUnitRule = { baseUnit: 'UNIT', fractionalUnitAllowed: false }
const fractionalUnit: QuantityUnitRule = { baseUnit: 'UNIT', fractionalUnitAllowed: true }

describe('validateRecipeQuantity', () => {
  it('requires a value', () => {
    expect(validateRecipeQuantity('', gram)).toBe('Ingresa una cantidad.')
  })

  it('rejects non-numeric and exponent strings', () => {
    expect(validateRecipeQuantity('abc', gram)).toBe('Usa un número con hasta 6 decimales.')
    expect(validateRecipeQuantity('1e3', gram)).toBe('Usa un número con hasta 6 decimales.')
  })

  it('requires strictly positive quantities', () => {
    expect(validateRecipeQuantity('0', gram)).toBe('La cantidad debe ser mayor que cero.')
    expect(validateRecipeQuantity('-1', gram)).toBe('La cantidad debe ser mayor que cero.')
  })

  it('accepts exact positive decimals', () => {
    expect(validateRecipeQuantity('18.5', gram)).toBeNull()
    expect(validateRecipeQuantity('18.500000', gram)).toBeNull()
    expect(validateRecipeQuantity('250', gram)).toBeNull()
  })

  it('rejects fractional quantities for discrete UNIT items but allows integers', () => {
    expect(validateRecipeQuantity('0.5', discreteUnit)).toBe('Este artículo no admite fracciones; usa una cantidad entera.')
    expect(validateRecipeQuantity('1', discreteUnit)).toBeNull()
    expect(validateRecipeQuantity('1.000000', discreteUnit)).toBeNull()
    expect(validateRecipeQuantity('1.5', fractionalUnit)).toBeNull()
  })
})

describe('validateEffectDelta', () => {
  it('requires a value', () => {
    expect(validateEffectDelta('', gram)).toBe('Ingresa un valor.')
  })

  it('rejects zero deltas', () => {
    expect(validateEffectDelta('0', gram)).toBe('El valor no puede ser cero.')
    expect(validateEffectDelta('0.000000', gram)).toBe('El valor no puede ser cero.')
  })

  it('accepts signed non-zero decimals', () => {
    expect(validateEffectDelta('-250', gram)).toBeNull()
    expect(validateEffectDelta('250.5', gram)).toBeNull()
  })

  it('rejects a leading plus sign that the API would not receive as text', () => {
    expect(validateEffectDelta('+5', gram)).toBe('Usa un número (puede ser negativo) con hasta 6 decimales.')
  })

  it('rejects fractional deltas for discrete UNIT items', () => {
    expect(validateEffectDelta('0.5', discreteUnit)).toBe('Este artículo no admite fracciones; usa una cantidad entera.')
    expect(validateEffectDelta('-0.5', discreteUnit)).toBe('Este artículo no admite fracciones; usa una cantidad entera.')
    expect(validateEffectDelta('-2', discreteUnit)).toBeNull()
  })
})
