import { URL } from 'node:url'

const required = ['APP_ENV', 'E2E_DB_URL', 'E2E_DB_USER', 'E2E_DB_PASSWORD']
const missing = required.filter((name) => !process.env[name])
if (missing.length) {
  console.error(`E2E safety check: missing ${missing.join(', ')}`)
  process.exit(1)
}

if (process.env.APP_ENV !== 'e2e') {
  console.error('E2E safety check: APP_ENV must be exactly "e2e".')
  process.exit(1)
}

const jdbc = process.env.E2E_DB_URL.match(/^jdbc:postgresql:\/\/([^/]+)\/([^?]+)(?:\?.*)?$/)
if (!jdbc) {
  console.error('E2E safety check: expected a PostgreSQL JDBC URL with an explicit database name.')
  process.exit(1)
}

let endpoint
try {
  endpoint = new URL(`http://${jdbc[1]}`)
} catch {
  console.error('E2E safety check: invalid database host.')
  process.exit(1)
}

const localHosts = new Set(['localhost', '127.0.0.1', '::1', '[::1]'])
const databaseName = decodeURIComponent(jdbc[2])
if (!localHosts.has(endpoint.hostname) || !databaseName.startsWith('hiqnet_pos_e2e')) {
  console.error('E2E safety check refused the database. Use a local host and a database named hiqnet_pos_e2e*.')
  process.exit(1)
}

console.log(`E2E safety check passed for local database ${databaseName}.`)
