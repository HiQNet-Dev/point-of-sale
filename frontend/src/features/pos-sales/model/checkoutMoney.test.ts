import { describe, expect, it } from 'vitest'
import {
  addExactMoney,
  compareExactMoney,
  exactMoneyToDecimal,
  formatExactMoney,
  formatExactMoneyUnits,
  parseExactMoney,
  subtractExactMoney,
} from './checkoutMoney'

describe('exact checkout money', () => {
  it.each([
    ['0', 0n],
    ['0.0', 0n],
    ['0.0000', 0n],
    ['1', 10_000n],
    ['1.2', 12_000n],
    ['1.2345', 12_345n],
    ['999999999999999.9999', 9_999_999_999_999_999_999n],
  ])('parses %s at scale four', (value, expected) => {
    expect(parseExactMoney(value)).toBe(expected)
  })

  it.each(['', ' ', '-1', '+1', '1.23456', '1e3', '1,25', '1000000000000000', 'NaN', 'Infinity', 'abc'])('rejects invalid decimal %s', (value) => {
    expect(parseExactMoney(value)).toBeNull()
  })

  it('adds, subtracts, compares and serializes amounts without floating point', () => {
    const applied = addExactMoney(parseExactMoney('40')!, parseExactMoney('60.0001')!)
    expect(exactMoneyToDecimal(applied)).toBe('100.0001')
    expect(exactMoneyToDecimal(subtractExactMoney(parseExactMoney('100')!, applied))).toBe('-0.0001')
    expect(exactMoneyToDecimal(addExactMoney(parseExactMoney('0.1')!, parseExactMoney('0.2')!))).toBe('0.3')
    expect(compareExactMoney(applied, parseExactMoney('100')!)).toBe(1)
  })

  it('formats exact fractional digits and never rounds a four-decimal amount', () => {
    expect(formatExactMoney('12.3400', 'en-US', 'USD')).toContain('12.34')
    expect(formatExactMoney('0.0001', 'en-US', 'USD')).toContain('0.0001')
    expect(formatExactMoneyUnits(-5_000n, 'en-US', 'USD')).toBe('-$0.50')
    expect(formatExactMoneyUnits(-12_345_000n, 'en-US', 'USD')).toContain('-$1,234.5')
    expect(formatExactMoneyUnits(-1n, 'en-US', 'USD')).toBe('-$0.0001')
    expect(formatExactMoney('999999999999999.9999', 'en-US', 'USD'))
      .toContain('999,999,999,999,999.9999')
    expect(formatExactMoney('12.3400', 'es-MX', 'MXN')).toContain('12.34')
  })
})
