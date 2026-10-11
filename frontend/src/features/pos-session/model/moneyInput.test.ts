import { describe, expect, it } from 'vitest'
import { validateAmount } from './moneyInput'

describe('validateAmount', () => {
  it.each(['0', '0.0000', '1', '1.2', '1.2345'])('accepts the exact decimal string %s', (value) => {
    expect(validateAmount(value)).toBeNull()
  })

  it.each(['', '   ', '-1', '+1', '1.23456', '.5', '1.', '1,25', '1e2', 'abc'])('rejects invalid monetary input %s', (value) => {
    expect(validateAmount(value)).not.toBeNull()
  })
})
