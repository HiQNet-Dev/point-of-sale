// E2E Windows del laboratorio: WebView2 real y registro Tauri real. Sin mockIPC.
import { fileURLToPath } from 'node:url'
import { expect } from '@playwright/test'
import { cleanup, close, connect, freePort, launch, temporaryProfile } from './desktop-e2e.mjs'

const port = await freePort()
const profile = await temporaryProfile('hiqnet-webview-e2e-')
const executable = fileURLToPath(new URL('../../backend-rust/apps/pos-desktop/target/debug/pos-desktop.exe', import.meta.url))
const running = launch({ executable, args: ['--synthetic-lab'], profileDirectory: profile, port })
let browser
try {
  browser = await connect({ child: running.child, port, launchError: running.launchError })
  const page = browser.contexts()[0].pages()[0]
  await expect(page.getByRole('heading', { name: 'Catálogo Rust · laboratorio IPC' })).toBeVisible()
  await expect(page.getByText('Sucursal Centro · centro · ACTIVE')).toBeVisible()
  await expect(page.getByText('Bebidas · bebidas')).toBeVisible()
  await expect(page.getByRole('cell', { name: 'Café americano', exact: true })).toBeVisible()
  await expect(page.getByRole('cell', { name: '12.3456', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Actualizar' }).click()
  await expect(page.getByRole('cell', { name: 'CAFE-1', exact: true })).toBeVisible()

  // Escritura real: React → binding → comando Tauri → aplicación → SQLite → auditoría.
  await page.getByLabel('SKU').fill('CAFE-2')
  await page.getByLabel('Nombre').fill('Café doble')
  await page.getByLabel('Precio (escala 4)').fill('9.9900')
  await page.getByRole('button', { name: 'Crear producto' }).click()
  await expect(page.getByText('Producto creado: CAFE-2')).toBeVisible()
  await expect(page.getByRole('cell', { name: 'CAFE-2', exact: true })).toBeVisible()
  await expect(page.getByRole('cell', { name: '9.9900', exact: true })).toBeVisible()

  // El mismo SKU devuelve conflicto tipado, sin duplicar la fila.
  await page.getByLabel('SKU').fill('CAFE-2')
  await page.getByLabel('Nombre').fill('Café doble')
  await page.getByLabel('Precio (escala 4)').fill('9.9900')
  await page.getByRole('button', { name: 'Crear producto' }).click()
  await expect(page.getByText('No se pudo crear el producto: CONFLICT')).toBeVisible()

  const invalidPage = await page.evaluate(async () => {
    try {
      await window.__TAURI_INTERNALS__.invoke('lab_list_products', { request: { limit: 0, offset: 0 } })
      return null
    } catch (error) { return error }
  })
  expect(invalidPage).toEqual({ code: 'VALIDATION_ERROR' })
  const overpostingRejected = await page.evaluate(async () => {
    try {
      await window.__TAURI_INTERNALS__.invoke('lab_list_products', { request: { limit: 20, offset: 0, organizationId: 'untrusted' } })
      return false
    } catch { return true }
  })
  expect(overpostingRejected).toBe(true)
  const noInstallation = await page.evaluate(async () => {
    try {
      await window.__TAURI_INTERNALS__.invoke('installation_status')
      return false
    } catch (error) { return error }
  })
  expect(noInstallation).toEqual({ code: 'OPERATION_NOT_AVAILABLE' })
} finally {
  const forcedClose = await close({ child: running.child, browser })
  await cleanup(profile)
  if (forcedClose) throw new Error('La aplicación no respondió al cierre normal de ventana')
}
console.log(JSON.stringify({ status: 'PASS', mode: 'synthetic_lab', transport: 'tauri_webview2_ipc', consumer: 'React', commands: 5, exactUnitPrice: '12.3456', writeVertical: 'create_product', conflictCode: 'CONFLICT', cleanShutdown: true, productive: false }))
