// E2E Windows del modo instalación: dos arranques sobre el mismo directorio.
// Demuestra persistencia real entre reinicios, sin sesiones ni datos sintéticos.
import { fileURLToPath } from 'node:url'
import { stat } from 'node:fs/promises'
import { join } from 'node:path'
import { expect } from '@playwright/test'
import { cleanup, close, connect, freePort, launch, temporaryProfile } from './desktop-e2e.mjs'

const executable = fileURLToPath(new URL('../../backend-rust/apps/pos-desktop/target/debug/pos-desktop.exe', import.meta.url))
const installation = await temporaryProfile('hiqnet-installation-e2e-')
const database = join(installation, 'hiqnet-pos.sqlite')

async function boot(expectedState) {
  const port = await freePort()
  const profile = await temporaryProfile('hiqnet-webview-installation-')
  const running = launch({ executable, args: ['--installation', installation], profileDirectory: profile, port })
  let browser
  try {
    browser = await connect({ child: running.child, port, launchError: running.launchError })
    const page = browser.contexts()[0].pages()[0]
    await expect(page.getByRole('heading', { name: 'HiQNet POS — Instalación Local' })).toBeVisible()
    await expect(page.getByRole('row', { name: /Apertura/ })).toContainText(expectedState)
    await expect(page.getByRole('row', { name: /Versión de esquema/ })).toContainText('1')
    // El modo instalación no sirve catálogo sintético.
    const catalog = await page.evaluate(async () => {
      try {
        await window.__TAURI_INTERNALS__.invoke('lab_list_products', { request: { limit: 20, offset: 0 } })
        return null
      } catch (error) { return error }
    })
    expect(catalog).toEqual({ code: 'OPERATION_NOT_AVAILABLE' })
  } finally {
    const forcedClose = await close({ child: running.child, browser })
    await cleanup(profile)
    if (forcedClose) throw new Error('La aplicación no respondió al cierre normal de ventana')
  }
}

try {
  await boot('Primera instalación')
  // El archivo debe seguir existiendo tras el cierre limpio.
  expect((await stat(database)).isFile()).toBe(true)
  await boot('Instalación reabierta')
  expect((await stat(database)).isFile()).toBe(true)
} finally {
  await cleanup(installation)
}
console.log(JSON.stringify({ status: 'PASS', mode: 'installation', transport: 'tauri_webview2_ipc', persistent: true, schemaVersion: 1, authEnabled: false, productive: false }))
