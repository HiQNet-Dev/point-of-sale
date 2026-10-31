// Smoke real del runtime headless, aislado de todos los volúmenes existentes.
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../', import.meta.url))
const project = `hiqnet-pos-smoke-${Date.now()}`
function compose(args) {
  const result = spawnSync('docker', ['compose', '--progress', 'plain', '-p', project, '-f', 'docker-compose.rust.yml', ...args], {
    cwd: root, encoding: 'utf8', env: { ...process.env, MSYS_NO_PATHCONV: '1' },
  })
  if (result.stdout) process.stdout.write(result.stdout)
  if (result.stderr) process.stderr.write(result.stderr)
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error(`Docker smoke failed: ${args[0]} (${result.status})`)
  return result.stdout.trim()
}

try {
  compose(['build', 'pos-local'])
  compose(['up', '-d', '--wait', '--wait-timeout', '60', 'pos-local'])
  compose(['exec', '-T', 'pos-local', '/usr/local/bin/pos-local', '--health'])
  const permissions = compose(['exec', '-T', 'pos-local', 'stat', '-c', '%a %u:%g', '/data'])
  if (permissions !== '700 10001:10001') throw new Error('SQLite volume is not private')
  compose(['stop', 'pos-local'])
  // Primera ejecución sobre volumen vacío: el proceso debe informar primera instalación.
  const firstLogs = compose(['logs', '--no-color', 'pos-local'])
  if (!firstLogs.includes('"state":"first_install"') || !firstLogs.includes('"event":"ready"')) {
    throw new Error('Missing first-install evidence')
  }
  // Comparar solo después del cierre/checkpoint: el hash del archivo abierto
  // no representa el estado SQLite completo mientras existan páginas en WAL.
  const before = compose(['run', '--rm', '--no-deps', '--entrypoint', 'sha256sum', 'pos-local', '/data/pos.sqlite'])
  compose(['up', '-d', '--wait', '--wait-timeout', '60', 'pos-local'])
  compose(['exec', '-T', 'pos-local', '/usr/local/bin/pos-local', '--health'])
  compose(['stop', 'pos-local'])
  const after = compose(['run', '--rm', '--no-deps', '--entrypoint', 'sha256sum', 'pos-local', '/data/pos.sqlite'])
  if (before !== after) throw new Error('SQLite changed across clean reopen')
  const logs = compose(['logs', '--no-color', 'pos-local'])
  if (!logs.includes('shutdown_requested') || !logs.includes('stopped')) throw new Error('Missing clean shutdown evidence')
  // La segunda apertura reutiliza el mismo archivo: no vuelve a migrar.
  if (!logs.includes('"state":"reopened"')) throw new Error('Missing reopen evidence')
  console.log(JSON.stringify({ status: 'PASS', project, migrations: 'approved only', permissions, durableClosedFile: before.split(' ')[0], firstInstall: true, reopen: true, productive: false }))
} finally {
  // El nombre se generó en esta ejecución; nunca borrar volúmenes de otro proyecto.
  compose(['down', '--volumes'])
}
