// Arranque explícito de la aplicación de escritorio; no abre HTTP ni el cliente Java.
//
//   pnpm desktop:lab                              laboratorio sintético efímero
//   pnpm desktop:install -- <directorio>          instalación persistente
//   pnpm generate:desktop-contracts               bindings TypeScript
//   pnpm build:rust-lab                           build sin ejecutar
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const frontend = fileURLToPath(new URL('../', import.meta.url))
const desktop = fileURLToPath(new URL('../../backend-rust/apps/pos-desktop/', import.meta.url))
function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit' })
  if (result.error) throw result.error
  if (result.status !== 0) process.exit(result.status ?? 1)
}
run(process.execPath, [fileURLToPath(new URL('../node_modules/vite/bin/vite.js', import.meta.url)), 'build', '--config', 'vite.rust-lab.config.ts'], frontend)
if (process.argv.includes('--build-only')) {
  run('cargo', ['build', '--locked'], desktop)
} else if (process.argv.includes('--export-bindings')) {
  run('cargo', ['run', '--locked', '--', '--export-bindings'], desktop)
} else {
  const installation = process.argv.indexOf('--installation')
  if (installation === -1) {
    run('cargo', ['run', '--locked', '--', '--synthetic-lab'], desktop)
  } else {
    const directory = process.argv[installation + 1]
    if (!directory || directory.startsWith('--')) {
      throw new Error('desktop:install requiere un directorio: pnpm desktop:install -- <directorio>')
    }
    run('cargo', ['run', '--locked', '--', '--installation', directory], desktop)
  }
}
