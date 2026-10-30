// Utilidades compartidas del E2E de escritorio: WebView2 real, sin mockIPC.
// Solo controla procesos hijos propios; nunca mata procesos por nombre.
import { spawn, spawnSync } from 'node:child_process'
import { mkdtemp, rm } from 'node:fs/promises'
import { createServer } from 'node:net'
import { once } from 'node:events'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { chromium } from '@playwright/test'

if (process.platform !== 'win32') throw new Error('Este E2E requiere Windows y WebView2; Docker no lo sustituye.')

/** Reserva un puerto efímero de loopback para DevTools del proceso de prueba. */
export async function freePort() {
  const server = createServer()
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const port = server.address().port
  await new Promise(resolve => server.close(resolve))
  return port
}

/** Perfil WebView2 temporal, aislado del perfil del usuario. */
export async function temporaryProfile(prefix) {
  return mkdtemp(join(tmpdir(), prefix))
}

/**
 * Lanza el ejecutable con un perfil temporal y un puerto DevTools efímero.
 * El puerto solo existe en el entorno del hijo de prueba.
 */
export function launch({ executable, args, profileDirectory, port }) {
  const child = spawn(executable, args, {
    env: {
      ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: profileDirectory,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  let launchError
  child.on('error', error => { launchError = error })
  child.stdout.on('data', data => process.stdout.write(data))
  child.stderr.on('data', data => process.stderr.write(data))
  return { child, get launchError() { return launchError } }
}

/** Espera a que el proceso propio publique DevTools y conecta por CDP. */
export async function connect({ child, port, launchError }) {
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
  return chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 10_000 })
}

/** Cierra la ventana de forma normal; fuerza el cierre solo como último recurso. */
export async function close({ child, browser }) {
  let forcedClose = false
  if (browser) await browser.close()
  if (child.pid && child.exitCode === null) {
    spawnSync('powershell.exe', ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid} -ErrorAction SilentlyContinue).CloseMainWindow()`], { stdio: 'ignore', timeout: 5000 })
    await Promise.race([once(child, 'exit'), new Promise(resolve => setTimeout(resolve, 5000))])
    if (child.exitCode === null) {
      forcedClose = true
      spawnSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', timeout: 5000 })
    }
  }
  return forcedClose
}

/** Elimina directorios temporales del propio script con reintentos en Windows. */
export async function cleanup(...directories) {
  for (const directory of directories) {
    await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 })
  }
}
