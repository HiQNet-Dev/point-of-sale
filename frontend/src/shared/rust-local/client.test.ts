/// <reference types="node" />
import { execFile } from 'node:child_process'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { promisify } from 'node:util'
import { describe, expect, it } from 'vitest'
import { createLabClient, parseScenarioReport } from './client'

const execute = promisify(execFile)
const workspace = resolve('../backend-rust')

// Invoca el binario Rust real, no una respuesta simulada ni un servidor HTTP.
describe.skipIf(process.env.RUST_CONTRACT_TESTS !== '1')('Rust candidate contract integration', () => {
  it('generates the checked-in bindings from Rust', async () => {
    const { stdout } = await execute('cargo', ['run', '--locked', '-q', '-p', 'pos-lab', '--', '--export-contract'], { cwd: workspace })
    expect(stdout.replaceAll('\r\n', '\n')).toBe(await readFile('src/shared/rust-local/generated.ts', 'utf8'))
  }, 120_000)

  it('invokes SQLite, checkout and reopen through the typed client', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'hiqnet-contract-'))
    try {
      const database = join(directory, 'scenario.sqlite')
      const client = createLabClient({
        async invoke(command) {
          expect(command).toBe('lab.runScenario')
          const { stdout } = await execute('cargo', ['run', '--locked', '-q', '-p', 'pos-lab', '--', '--database', database, '--contract-report'], { cwd: workspace })
          return JSON.parse(stdout) as unknown
        },
      })
      const report = await client.runScenario()
      expect(report.saleTotal).toBe('24.6912')
      expect(report.changeAmount).toBe('5.3088')
      expect(report.inventoryBalance).toBe('500.000000')
      expect(report.openSalePreventedClose).toBe(true)
      expect(report.productive).toBe(false)
      expect(() => parseScenarioReport({ ...report, saleTotal: 24.6912 })).toThrow('INVALID_LAB_RESPONSE')
      expect(() => parseScenarioReport({ ...report, secret: 'unexpected' })).toThrow('INVALID_LAB_RESPONSE')
      expect(() => parseScenarioReport({ ...report, sale_total: report.saleTotal, saleTotal: undefined })).toThrow('INVALID_LAB_RESPONSE')
      expect(() => parseScenarioReport({ ...report, productive: true })).toThrow('INVALID_LAB_RESPONSE')
    } finally {
      await rm(directory, { recursive: true, force: true })
    }
  }, 120_000)

  it('refuses an existing database without destroying its contents', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'hiqnet-sentinel-'))
    const database = join(directory, 'existing.sqlite')
    try {
      await writeFile(database, 'DO NOT DELETE')
      await expect(execute('cargo', ['run', '--locked', '-q', '-p', 'pos-lab', '--', '--database', database, '--contract-report'], { cwd: workspace })).rejects.toThrow()
      expect(await readFile(database, 'utf8')).toBe('DO NOT DELETE')
    } finally {
      await rm(directory, { recursive: true, force: true })
    }
  }, 120_000)
})
