// E2E Windows: WebView2 real y registro Tauri real. No usa mockIPC ni pos-lab CLI.
import { spawn, spawnSync } from 'node:child_process'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createServer } from 'node:net'
import { fileURLToPath } from 'node:url'
import { once } from 'node:events'
import { chromium, expect } from '@playwright/test'

if (process.platform !== 'win32') throw new Error('Este E2E requiere Windows y WebView2; Docker no lo sustituye.')
const server = createServer()
server.listen(0, '127.0.0.1')
await once(server, 'listening')
const port = server.address().port
await new Promise(resolve => server.close(resolve))
const directory = await mkdtemp(join(tmpdir(), 'hiqnet-webview-e2e-'))
const executable = fileURLToPath(new URL('../../backend-rust/tools/desktop-lab/target/debug/pos-desktop-lab.exe', import.meta.url))
const child = spawn(executable, ['--synthetic-lab'], {
  env: {
    ...process.env,
    WEBVIEW2_USER_DATA_FOLDER: directory,
    // DevTools local exclusivo del proceso de prueba; no forma parte del runtime.
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  },
  stdio: ['ignore', 'pipe', 'pipe'],
})
let launchError
child.on('error', error => { launchError = error })
child.stdout.on('data', data => process.stdout.write(data))
child.stderr.on('data', data => process.stderr.write(data))
let browser
try {
  const deadline = Date.now() + 60_000
  while (Date.now() < deadline) {
    if (launchError) throw launchError
    if (child.exitCode !== null) throw new Error(`La aplicación terminó antes de iniciar: ${child.exitCode}`)
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`)
      if (response.ok) break
    } catch { /* Esperar únicamente al proceso propio. */ }
    await new Promise(resolve => setTimeout(resolve, 200))
  }
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 10_000 })
  const page = browser.contexts()[0].pages()[0]
  await expect(page.getByRole('heading', { name: 'Catálogo Rust · laboratorio IPC' })).toBeVisible()
  await expect(page.getByText('Sucursal Centro · centro · ACTIVE')).toBeVisible()
  await expect(page.getByText('Bebidas · bebidas')).toBeVisible()
  await expect(page.getByRole('cell', { name: 'Café americano', exact: true })).toBeVisible()
  await expect(page.getByRole('cell', { name: '12.3456', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Actualizar' }).click()
  await expect(page.getByRole('cell', { name: 'CAFE-1', exact: true })).toBeVisible()
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
} finally {
  let forcedClose = false
  if (browser) await browser.close()
  if (child.pid && child.exitCode === null) {
    // Cierre de ventana del único proceso creado por este script, no procesos por nombre.
    spawnSync('powershell.exe', ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid} -ErrorAction SilentlyContinue).CloseMainWindow()`], { stdio: 'ignore', timeout: 5000 })
    await Promise.race([once(child, 'exit'), new Promise(resolve => setTimeout(resolve, 5000))])
    if (child.exitCode === null) {
      forcedClose = true
      spawnSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', timeout: 5000 })
    }
  }
  await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 })
  if (forcedClose) throw new Error('La aplicación no respondió al cierre normal de ventana')
}
console.log(JSON.stringify({ status: 'PASS', transport: 'tauri_webview2_ipc', consumer: 'React', commands: 3, exactUnitPrice: '12.3456', cleanShutdown: true, productive: false }))
