import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

// Ejecución explícita: exige Rust, no añade cargo a la suite frontend ordinaria.
const frontend = fileURLToPath(new URL('../', import.meta.url))
const vitest = fileURLToPath(new URL('../node_modules/vitest/vitest.mjs', import.meta.url))
const result = spawnSync(process.execPath, [vitest, 'run', 'src/shared/rust-local/client.test.ts'], {
  cwd: frontend,
  env: { ...process.env, RUST_CONTRACT_TESTS: '1' },
  stdio: 'inherit',
})
if (result.error) console.error('No se pudo iniciar la suite de contratos Rust.')
process.exit(result.status ?? 1)
