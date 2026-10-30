// Interoperabilidad criptográfica Rust ↔ Java con credenciales sintéticas.
//
// Comprueba en ambas direcciones:
//   1. Java (BCryptPasswordEncoder) genera → Rust verifica.
//   2. Rust (Argon2id) genera             → Java (Argon2PasswordEncoder) verifica.
//
// Requiere JDK y los jars de Spring Security ya presentes en el repositorio Maven
// local. No descarga nada y no abre red. Los secretos son sintéticos.
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const backendRust = fileURLToPath(new URL('../../', import.meta.url))
const repository = join(homedir(), '.m2', 'repository')
const separator = process.platform === 'win32' ? ';' : ':'

/** Vectores sintéticos; nunca son credenciales reales. */
const vectors = [
  { label: 'cost10', password: 'synthetic-interop-vector-1', cost: 10 },
  { label: 'cost12', password: 'synthetic-interop-vector-2', cost: 12 },
]
const WRONG_PASSWORD = 'synthetic-wrong-password'

function latestJar(groupPath, artifact) {
  const base = join(repository, groupPath, artifact)
  if (!existsSync(base)) return null
  const versions = readdirSync(base, { withFileTypes: true })
    .filter(entry => entry.isDirectory())
    .map(entry => entry.name)
    .sort((left, right) => left.localeCompare(right, undefined, { numeric: true }))
  for (const version of versions.reverse()) {
    const directory = join(base, version)
    const jar = readdirSync(directory).find(name => name.endsWith('.jar') && !name.endsWith('-sources.jar'))
    if (jar) return join(directory, jar)
  }
  return null
}

function fail(message) {
  console.error(`interop: ${message}`)
  process.exit(1)
}

const classpathEntries = [
  latestJar('org/springframework/security', 'spring-security-crypto'),
  latestJar('org/bouncycastle', 'bcprov-jdk18on'),
  latestJar('org/springframework', 'spring-core'),
  latestJar('org/springframework', 'spring-jcl'),
].filter(Boolean)
if (classpathEntries.length < 3) fail('faltan jars de Spring Security en el repositorio Maven local')
const classpath = classpathEntries.join(separator)

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, ...options })
  if (result.error) fail(`${command}: ${result.error.message}`)
  if (result.status !== 0) fail(`${command} salió con ${result.status}: ${(result.stderr || '').trim()}`)
  return result.stdout
}

const workspace = mkdtempSync(join(tmpdir(), 'hiqnet-credential-interop-'))
const javaSource = fileURLToPath(new URL('./CredentialVectors.java', import.meta.url))
try {
  run('javac', ['-encoding', 'UTF-8', '-cp', classpath, '-d', workspace, javaSource])
  const java = (mode, options = {}) => run('java', ['-Dfile.encoding=UTF-8', '-cp', `${workspace}${separator}${classpath}`, 'CredentialVectors', mode], options)
  const rust = (mode, input) => run('cargo', ['run', '--quiet', '--locked', '-p', 'pos-credentials', '--example', 'credential_interop', '--', mode], {
    cwd: backendRust,
    input,
  })

  // 1. Java BCrypt → Rust.
  const bcryptOutput = java('bcrypt', {
    input: vectors.map(vector => `${vector.label}\t${vector.password}\t${vector.cost}`).join('\n'),
  })
  const bcryptHashes = new Map(bcryptOutput.split(/\r?\n/).filter(Boolean).map(line => {
    const [label, hash] = line.split('\t')
    return [label, hash]
  }))
  if (bcryptHashes.size !== vectors.length) fail('Java no devolvió todos los hashes BCrypt')

  const rustBcrypt = rust('verify-bcrypt', vectors
    .flatMap(vector => [
      `${vector.label}-ok\t${vector.password}\t${bcryptHashes.get(vector.label)}`,
      `${vector.label}-bad\t${WRONG_PASSWORD}\t${bcryptHashes.get(vector.label)}`,
    ])
    .join('\n'))
  const bcryptResults = new Map(rustBcrypt.split(/\r?\n/).filter(Boolean).map(line => line.split('\t')))

  // 2. Rust Argon2id → Java.
  const rustArgon = rust('emit-argon2', vectors.map(vector => `${vector.label}\t${vector.password}`).join('\n'))
  const argonHashes = new Map(rustArgon.split(/\r?\n/).filter(Boolean).map(line => {
    const [label, hash] = line.split('\t')
    return [label, hash]
  }))
  if (argonHashes.size !== vectors.length) fail('Rust no devolvió todos los hashes Argon2id')

  const javaArgon = java('verify-argon2', {
    input: vectors
      .flatMap(vector => [
        `${vector.label}-ok\t${vector.password}\t${argonHashes.get(vector.label)}`,
        `${vector.label}-bad\t${WRONG_PASSWORD}\t${argonHashes.get(vector.label)}`,
      ])
      .join('\n'),
  })
  const argonResults = new Map(javaArgon.split(/\r?\n/).filter(Boolean).map(line => {
    const [label, value] = line.split('\t')
    return [label, (value ?? '').trim()]
  }))

  const checks = []
  for (const vector of vectors) {
    checks.push({
      check: `rust_verifies_java_bcrypt_${vector.label}`,
      passed: bcryptResults.get(`${vector.label}-ok`) === 'VERIFIED_REHASH',
      observed: bcryptResults.get(`${vector.label}-ok`) ?? null,
    })
    checks.push({
      check: `rust_rejects_java_bcrypt_${vector.label}_with_wrong_password`,
      passed: bcryptResults.get(`${vector.label}-bad`) === 'REJECTED',
      observed: bcryptResults.get(`${vector.label}-bad`) ?? null,
    })
    checks.push({
      check: `java_verifies_rust_argon2_${vector.label}`,
      passed: argonResults.get(`${vector.label}-ok`) === 'true',
      observed: argonResults.get(`${vector.label}-ok`) ?? null,
    })
    checks.push({
      check: `java_rejects_rust_argon2_${vector.label}_with_wrong_password`,
      passed: argonResults.get(`${vector.label}-bad`) === 'false',
      observed: argonResults.get(`${vector.label}-bad`) ?? null,
    })
  }
  const formatChecks = [
    { check: 'java_bcrypt_uses_legacy_prefix', passed: [...bcryptHashes.values()].every(hash => hash.startsWith('$2a$')) },
    { check: 'rust_argon2_uses_phc_v19', passed: [...argonHashes.values()].every(hash => hash.startsWith('$argon2id$v=19$')) },
  ]

  const all = [...checks, ...formatChecks]
  const report = {
    status: all.every(check => check.passed) ? 'PASS' : 'FAIL',
    direction: ['java_bcrypt_to_rust', 'rust_argon2_to_java'],
    policies: { rust: 'lab-argon2-v1', java: 'Argon2PasswordEncoder(16,32,1,19456,2)' },
    classpath: classpathEntries.map(entry => entry.split(/[\\/]/).slice(-1)[0]),
    checks: all,
    normative: 'D-AUTH-09 remains blocked pending target-hardware benchmark and holder approval',
  }
  const target = join(backendRust, 'target')
  mkdirSync(target, { recursive: true })
  writeFileSync(join(target, '1z-credential-interop.json'), `${JSON.stringify(report, null, 2)}\n`, 'utf8')
  console.log(JSON.stringify(report, null, 2))
  if (report.status !== 'PASS') fail('al menos una comprobación de interoperabilidad falló')
} finally {
  rmSync(workspace, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 })
}
