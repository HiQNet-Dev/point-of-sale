// Arranque explícito del laboratorio; no cambia el cliente Java ni abre HTTP.
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const frontend = fileURLToPath(new URL('../', import.meta.url))
const desktop = fileURLToPath(new URL('../../backend-rust/tools/desktop-lab/', import.meta.url))
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
  run('cargo', ['run', '--locked', '--', '--synthetic-lab'], desktop)
}
