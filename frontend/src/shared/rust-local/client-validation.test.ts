import { describe, expect, it } from 'vitest'
import { createLabClient, parseScenarioReport, RustLocalError } from './client'
import type { ScenarioReport } from './generated'

// Ejemplo de contrato, no un sustituto de la prueba SQLite de integración.
const example = {
  productive: false,
  scope: 'synthetic_sqlite_candidate_schema',
  saleId: '00000000-0000-0000-0000-000000000000',
  saleTotal: '999999999999999.9999',
  changeAmount: '0.0000',
  saleStatus: 'COMPLETED',
  cashShiftStatus: 'CLOSED',
  inventoryBalance: '-0.000001',
  openSalePreventedClose: true,
} satisfies ScenarioReport

describe('Rust Local boundary validation', () => {
  it('sanitizes transport failures using a generated error code', async () => {
    const client = createLabClient({
      async invoke() { throw new Error('private SQL diagnostics') },
    })
    await expect(client.runScenario()).rejects.toMatchObject({ code: 'LAB_SCENARIO_FAILED' })
    await expect(client.runScenario()).rejects.toBeInstanceOf(RustLocalError)
    await expect(client.runScenario()).rejects.not.toThrow('private SQL diagnostics')
  })
  it('preserves exact decimal strings above JavaScript safe integer precision', () => {
    expect(parseScenarioReport(example).saleTotal).toBe('999999999999999.9999')
  })

  it.each([
    null,
    [],
    {},
    { ...example, saleTotal: 12.34 },
    { ...example, saleTotal: '1e4' },
    { ...example, saleTotal: '1.00001' },
    { ...example, changeAmount: '-1.0000' },
    { ...example, inventoryBalance: '0.1' },
    { ...example, productive: true },
    { ...example, saleStatus: 'OPEN' },
    { ...example, openSalePreventedClose: false },
    { ...example, saleId: 'not-a-uuid' },
    { ...example, sessionHandle: 'must-not-cross-this-boundary' },
  ])('rejects an incompatible response (%#)', (value) => {
    expect(() => parseScenarioReport(value)).toThrow('INVALID_LAB_RESPONSE')
  })
})
