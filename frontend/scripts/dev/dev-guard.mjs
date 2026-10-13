// Shared safety guards for the development seed/reset scripts.
//
// These scripts are DEVELOPMENT-ONLY. They refuse to run outside
// APP_ENV=development, against a non-"hiqnet_pos" database, against an E2E
// database, or against a non-local host (loopback, or the Compose service
// names reachable only inside the development network).
import { existsSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

/** Repository root (.../point-of-sale). */
export const REPO_ROOT = fileURLToPath(new URL('../../../', import.meta.url))
/** Absolute path to the repository's docker-compose.yml (works from any cwd). */
export const COMPOSE_FILE = fileURLToPath(new URL('../../../docker-compose.yml', import.meta.url))

/** Loopback hosts. */
export const LOCAL_HOSTS = new Set(['localhost', '127.0.0.1', '::1', '[::1]'])
/** Compose service names, valid only inside the development network. */
export const COMPOSE_HOSTS = new Set(['db', 'backend'])

/** Load a root `.env` file into process.env without overriding existing values. */
function loadRootEnv() {
  const file = `${REPO_ROOT}.env`
  if (!existsSync(file)) return
  for (const raw of readFileSync(file, 'utf8').split(/\r?\n/)) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    const eq = line.indexOf('=')
    if (eq === -1) continue
    const key = line.slice(0, eq).trim()
    let value = line.slice(eq + 1).trim()
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1)
    }
    if (!(key in process.env)) process.env[key] = value
  }
}

// Load the repository `.env` before any caller reads DEV_* / POSTGRES_* values.
loadRootEnv()

/** Development database name (never an E2E name). */
export const DEV_DB_NAME = process.env.POSTGRES_DB ?? process.env.DB_NAME ?? 'hiqnet_pos'
/** Development database user. */
export const DEV_DB_USER = process.env.POSTGRES_USER ?? process.env.DB_USER ?? 'hiqnet_dev'

/** Signal error carrying the exact [dev] message; handled at the top level. */
export class DevAbort extends Error {}

/** Prints a failure and aborts the current flow (never a hard process.exit). */
export function fail(message) {
  process.exitCode = 1
  throw new DevAbort(message)
}

/** Single top-level handler: prints the message, keeps a non-zero exit code. */
export function handleTopLevel(error) {
  if (error instanceof DevAbort) {
    console.error(`[dev] ${error.message}`)
    return
  }
  console.error(`[dev] ${error instanceof Error ? error.stack ?? error.message : String(error)}`)
  process.exitCode = 1
}

/** Requires APP_ENV to be exactly "development" (and never "prod"/"e2e"). */
export function requireDevelopment() {
  const env = process.env.APP_ENV
  if (env !== 'development') {
    fail(
      `Refusing to run: APP_ENV must be exactly "development" (got ${JSON.stringify(env)}). ` +
        'Copy .env.example to .env first (see docs/05-development/local-development.md).',
    )
  }
  if ((process.env.SPRING_PROFILES_ACTIVE ?? 'dev') === 'prod') {
    fail('Refusing to run: SPRING_PROFILES_ACTIVE must not be "prod".')
  }
}

/** Requires a local host for a raw URL or hostname (loopback or Compose service). */
export function assertLocalHost(raw, label) {
  const hostname = hostOf(raw)
  if (!hostname) {
    fail(`Refusing to run: ${label} is not a valid URL/host (got ${raw}).`)
  }
  if (!LOCAL_HOSTS.has(hostname) && !COMPOSE_HOSTS.has(hostname)) {
    fail(`Refusing to run: ${label} must be local (loopback or a Compose service), got ${hostname}.`)
  }
  return hostname
}

/** Requires the exact development database name and rejects any E2E database. */
export function assertDevDatabaseName(name = DEV_DB_NAME) {
  if (name.includes('e2e')) {
    fail(`Refusing to run against an E2E database (${name}).`)
  }
  if (name !== 'hiqnet_pos') {
    fail(`Refusing to run: development database must be exactly "hiqnet_pos" (got ${name}).`)
  }
  return name
}

function hostOf(raw) {
  try {
    return new URL(raw.includes('://') ? raw : `http://${raw}`).hostname
  } catch {
    return null
  }
}
