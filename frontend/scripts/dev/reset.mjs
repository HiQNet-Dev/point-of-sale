// Guarded DEVELOPMENT database reset for HiQNet POS.
//
// Drops and recreates ONLY the local development database inside the Compose
// `db` service, restarts the backend so Flyway re-applies V1..V21 on an empty
// schema, then re-runs the deterministic development seed. It refuses to run
// outside APP_ENV=development, against a non-"hiqnet_pos" database, against an
// E2E database, or against a non-local host.
//
// Usage:
//   pnpm dev:reset              # reset + reseed
//   pnpm dev:reset -- --no-seed # reset only (empty migrated database)
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import {
  COMPOSE_FILE,
  DEV_DB_NAME,
  DEV_DB_USER,
  REPO_ROOT,
  assertDevDatabaseName,
  assertLocalHost,
  fail,
  handleTopLevel,
  requireDevelopment,
} from './dev-guard.mjs'

const seedPath = fileURLToPath(new URL('./seed.mjs', import.meta.url))
const apiBase = process.env.DEV_API_URL ?? 'http://localhost:8080/api/v1'
const withSeed = !process.argv.includes('--no-seed')

function compose(args) {
  const result = spawnSync('docker', ['compose', '-f', COMPOSE_FILE, ...args], {
    cwd: REPO_ROOT,
    stdio: 'inherit',
  })
  if (result.error) fail(`Failed to run docker compose: ${result.error.message}`)
  if (result.status !== 0) fail(`docker compose ${args.join(' ')} exited with status ${result.status}.`)
}

function composeQuiet(args) {
  return spawnSync('docker', ['compose', '-f', COMPOSE_FILE, ...args], { cwd: REPO_ROOT, stdio: 'ignore' })
}

async function waitForBackend(timeoutMs = 180_000) {
  const healthUrl = `${apiBase.replace(/\/api\/v1$/, '')}/actuator/health`
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    try {
      if ((await fetch(healthUrl)).ok) return
    } catch {
      // keep waiting
    }
    await new Promise((resolve) => setTimeout(resolve, 2000))
  }
  fail(`Backend was not healthy at ${healthUrl} after ${timeoutMs / 1000}s.`)
}

async function main() {
  requireDevelopment()
  assertDevDatabaseName(DEV_DB_NAME)
  assertLocalHost(apiBase, 'DEV_API_URL')

  console.log(`[dev] Resetting development database "${DEV_DB_NAME}" in the Compose "db" service.`)
  compose(['up', '-d', 'db', 'backend'])

  const dbDeadline = Date.now() + 60_000
  for (;;) {
    if (composeQuiet(['exec', '-T', 'db', 'pg_isready', '-U', DEV_DB_USER, '-d', DEV_DB_NAME]).status === 0) break
    if (Date.now() > dbDeadline) fail(`PostgreSQL database "${DEV_DB_NAME}" did not become ready in 60s.`)
    await new Promise((resolve) => setTimeout(resolve, 2000))
  }

  compose([
    'exec',
    '-T',
    'db',
    'psql',
    '-U',
    DEV_DB_USER,
    '-d',
    'postgres',
    '-v',
    'ON_ERROR_STOP=1',
    '-c',
    `DROP DATABASE IF EXISTS "${DEV_DB_NAME}" WITH (FORCE);`,
    '-c',
    `CREATE DATABASE "${DEV_DB_NAME}" OWNER "${DEV_DB_USER}";`,
  ])

  compose(['restart', 'backend'])
  await waitForBackend()

  if (withSeed) {
    const seed = spawnSync(process.execPath, [seedPath], { cwd: REPO_ROOT, stdio: 'inherit' })
    if (seed.status !== 0) fail('Development seed failed after reset.')
    console.log('[dev] Development database reset and reseeded.')
  } else {
    console.log('[dev] Development database reset (empty, migrated). Run `pnpm dev:seed` to restore fixtures.')
  }
}

main().catch(handleTopLevel)
