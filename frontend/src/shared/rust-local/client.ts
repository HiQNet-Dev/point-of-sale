import { labCommand, scenarioReportSchema, type LabErrorCode, type ScenarioReport } from './generated'

/** Error estable sin datos SQL, credenciales ni stderr del proceso. */
export class RustLocalError extends Error {
  constructor(readonly code: LabErrorCode) {
    super(code)
    this.name = 'RustLocalError'
  }
}

/** Adaptador inyectable: solo el runner de pruebas implementa esta invocación. */
export interface LabTransport {
  invoke(command: typeof labCommand): Promise<unknown>
}

/** Rechaza respuestas incompatibles antes de entregarlas a los consumidores. */
export function parseScenarioReport(value: unknown): ScenarioReport {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new RustLocalError('INVALID_LAB_RESPONSE')
  }
  const record = value as Record<string, unknown>
  const properties = scenarioReportSchema.properties
  if (Object.keys(record).length !== Object.keys(properties).length) {
    throw new RustLocalError('INVALID_LAB_RESPONSE')
  }
  for (const key of scenarioReportSchema.required) {
    if (!Object.hasOwn(record, key) || typeof record[key] !== properties[key].type) {
      throw new RustLocalError('INVALID_LAB_RESPONSE')
    }
  }
  const report = record as unknown as ScenarioReport
  if (report.productive !== false || report.scope !== 'synthetic_sqlite_candidate_schema'
    || report.saleStatus !== 'COMPLETED' || report.cashShiftStatus !== 'CLOSED'
    || !report.openSalePreventedClose
    || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(report.saleId)
    || !/^\d+\.\d{4}$/.test(report.saleTotal)
    || !/^\d+\.\d{4}$/.test(report.changeAmount)
    || !/^-?\d+\.\d{6}$/.test(report.inventoryBalance)) {
    throw new RustLocalError('INVALID_LAB_RESPONSE')
  }
  return report
}

/** No exporta invoke de Tauri ni reutiliza el cliente HTTP Java. */
export function createLabClient(transport: LabTransport) {
  return {
    async runScenario(): Promise<ScenarioReport> {
      let response: unknown
      try {
        response = await transport.invoke(labCommand)
      } catch {
        throw new RustLocalError('LAB_SCENARIO_FAILED')
      }
      return parseScenarioReport(response)
    },
  }
}
