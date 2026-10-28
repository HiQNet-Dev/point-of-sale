# Estrategia de CI

Documentación de cómo GitHub Actions valida el repositorio. Hay **dos niveles**:

- **CI general** — para pull requests y para pushes a `dev`. Verifica código limpio e integración de código: backend Java, Rust,
  frontend y compose. **No** ejecuta la integración E2E.
- **CI de `main`** — más robusto y completo. Se ejecuta al hacer push a `main`
  (y en la validación manual) y añade la integración E2E real (Playwright +
  PostgreSQL) además de lo anterior.

| Contexto                            | Backend Java | Rust      | Frontend  | Compose   | E2E    |
| ----------------------------------- | ------------ | --------- | --------- | --------- | ------ |
| Pull request (hacia `dev` o `main`) | si aplica    | si aplica | si aplica | si aplica | no     |
| Push a `dev`                        | si aplica    | si aplica | si aplica | si aplica | no     |
| Push a `main`                       | si aplica    | si aplica | si aplica | si aplica | **sí** |
| Manual (`workflow_dispatch`)        | sí           | sí        | sí        | sí        | **sí** |

Los pushes disparan el workflow solo en `main` y `dev`; las ramas de trabajo se
validan mediante PR. En `main`, los jobs por área conservan la detección de
cambios existente; el E2E se ejecuta independientemente de esa detección.

No se eliminan ni se marcan como `skip` pruebas: la suite completa sigue
existiendo y se ejecuta en el nivel que corresponde.

## Detección de cambios

El job `Detect Changes` (`.github/scripts/ci-changes.sh`, solo `git`, sin actions
de terceros) clasifica los archivos modificados y publica los flags que consumen
los demás jobs:

| Área       | Rutas (aproximadas)                                                               |
| ---------- | --------------------------------------------------------------------------------- |
| `backend`  | `backend/**` (Java)                                                               |
| `rust`     | `backend-rust/**` (workspace, manifests, `Cargo.lock`, toolchain y fuentes)       |
| `frontend` | `frontend/**`                                                                     |
| docker     | `docker-compose.yml`, `Dockerfile*`, `.dockerignore`, `.env.example`, `deploy/**` |
| ci         | `.github/**`                                                                      |

Reglas de decisión:

- `docs`-only → no ejecuta jobs ejecutables (solo `Detect Changes` + `CI Gate`).
- cambio ejecutable de `backend`, `rust`, `frontend` o `docker` → ejecuta el job
  afín; `.github/**` ejecuta todos los jobs por área, incluido Rust.
- `rust` indica cambios directos del workspace; `rust_run` indica si el job es
  obligatorio (`rust`, `ci` o `full`). No se infiere Rust a partir del flag Java.
- La clasificación docs-only se conserva también para Markdown dentro del
  workspace; un cambio solo en `backend-rust/README.md` no ejecuta Rust.
- `workflow_dispatch` → validación completa.
- sin base de diff válida (por ejemplo, primer push) → se cae a validación completa.
- ruta desconocida (no documentación y no mapeada a un área) → fail-safe:
  validación completa, para no omitir jobs ante código no reconocido.

La integración E2E **no** depende de la detección de cambios: corre solo en el CI
robusto de `main` y en la validación manual.

## Rust: checks técnicos y gate normativo obligatorio

El job `rust` (`Rust (technical checks + mandatory DDL13 gate)`)
trabaja en `backend-rust`, con toolchain **1.99.1**, `rustfmt` y `clippy`.
La foundation declara MSRV **1.99**; este job valida 1.99.1, no una matriz
adicional de compiladores. El workspace contiene `crates/pos-core`,
`crates/pos-storage-sqlite`, `apps/pos-local` y `tools/migration-lab`.
`Cargo.lock` debe estar versionado: CI comprueba su presencia en Git y no lo
regenera ni permite resolver silenciosamente nuevas versiones.

Checks técnicos exactos, desde `backend-rust`:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo build --locked --workspace --all-features
sh tools/migration-lab/scripts/build_postgres.sh
cargo run --locked -p migration-lab -- --output target/migration-evidence \
  --postgres-bin target/pg17-build/install/bin --java-source
python3 tools/migration-lab/scripts/verify_evidence.py \
  --evidence target/migration-evidence --expect-exit 0
cargo install cargo-audit --version 0.22.2 --locked
cargo audit --file Cargo.lock --json > target/cargo-audit.json
```

El job configura Java 21 y Maven para el origen Flyway; PostgreSQL 17.11 se
compila desde fuente oficial con SHA-256 fijado. No usa Docker, `sudo`, el
servidor PostgreSQL del runner ni `DATABASE_URL`. `cargo test` no requiere
PostgreSQL: la integración real se ejecuta por el CLI y se valida con
`verify_evidence.py`.

La ejecución normal devuelve **0 por ejecución técnica correcta, NO por
aprobación normativa**. Los 41 checks técnicos pueden pasar, pero eso no basta
para que el job Rust ni `CI Gate` estén verdes. No hay `continue-on-error` a
nivel de job ni de step, `|| true`, fixtures aprobatorias ni resultados ficticios
de completitud.

### Gate normativo automático obligatorio: esquema / DDL13

La foundation ya incluye un primer esquema parcial: Local org/branch y PG
tenant/customer. Una revisión humana IMP01 por sí sola **no basta** para aprobar
eso alcance con obligatorios pendientes. **PAR-08 está aprobado** porque el
origen Flyway V1–V25 se ejecutó dos veces con 37/37 tablas y digest funcional
idéntico; los otros **29 casos siguen BLOCKED/NOT_RUN**. Pasar formato, Clippy,
tests, build o auditoría no permite aprobar el resto.

El último check del job, después de `cargo audit` e inmediatamente antes del
upload, ejecuta obligatoriamente:

```bash
cargo run --locked -p migration-lab -- --output target/migration-gate-evidence --postgres-bin target/pg17-build/install/bin --java-source --require-complete
python3 tools/migration-lab/scripts/verify_evidence.py --evidence target/migration-gate-evidence --expect-exit 2
```

Se ejecuta con `if: always()` dentro del job seleccionado por `rust_run=true`,
también si falla un check técnico anterior. Escribe en una salida distinta de
`target/migration-evidence`, sin sobrescribir la evidencia técnica parcial.
Mientras haya obligatorios pendientes, el laboratorio devuelve **exit 2
(`BLOCKED`)**, que se propaga sin interceptarse: el **job Rust queda rojo y
`CI Gate` falla**. Si una etapa anterior impide ejecutar el laboratorio, tampoco
se aprueba el job ni se fabrica evidencia.

**No hay merge hasta completar DDL13 y todos los obligatorios normativos.**
El bloqueo automático está activo también durante la foundation parcial: es un
bloqueo real e intencional, no un fallo oculto, opcional ni sustituible por una
aprobación humana IMP01. Solo la evidencia completa y satisfactoria del SHA
validado permite superar el gate; la revisión humana sigue siendo adicional.
`CI Gate` debe mantenerse como required status check en las reglas de protección
de rama, cuya configuración está fuera de estos archivos.

### Seguridad y artefactos Rust

- Secretos: Gitleaks **8.24.3**, binario Linux x64 verificado con SHA-256 de la
  [publicación oficial](https://github.com/gitleaks/gitleaks/releases/tag/v8.24.3),
  ejecuta `gitleaks dir . --redact --no-banner` sobre `backend-rust` antes de
  compilar. Es un escaneo del árbol del workspace, **no** de todo el historial ni
  de las otras áreas. No requiere tokens ni credenciales; los hallazgos fallan CI
  y se redactan en logs. No se agregan allowlists ni excepciones.
- Dependencias: `cargo-audit` **0.22.2**, con MSRV **1.88** según
  [crates.io](https://crates.io/api/v1/crates/cargo-audit/0.22.2).
  El pin se actualiza por el fallo de 0.21.2 al leer CVSS 4.0 en
  `RUSTSEC-2026-0076`; no se degrada ni se fija una base RustSec antigua.
  Se instala con `--locked` (lockfile de la herramienta); la auditoría inspecciona
  explícitamente el `Cargo.lock` del workspace. `cargo audit` no tiene un flag
  `--locked`: se usa `--file Cargo.lock`, sin `--ignore`, sin modo offline y sin
  excepciones inventadas. Vulnerabilidades y errores de auditoría fallan el job.
- Las descargas de herramientas/dependencias y de la base RustSec requieren red;
  no se envían datos de producción ni se usan secretos de producción.
- `actions/upload-artifact@v4` con **`if: always()`** intenta subir ambas salidas,
  `backend-rust/target/migration-evidence` (técnica parcial) y
  `backend-rust/target/migration-gate-evidence` (evaluación normativa), además de
  `backend-rust/target/cargo-audit.json`, como
  `rust-migration-technical-and-normative-evidence`, con 7 días de retención,
  también cuando el gate devuelve exit 2. Si una etapa anterior impide producir
  archivos, advierte de su ausencia; no fabrica evidencia ni convierte un
  resultado `BLOCKED` en éxito. Subir el artefacto no certifica la aprobación.

## Tests de la lógica de CI

`Detect Changes` ejecuta antes de clasificar:

```bash
python3 .github/scripts/test_ci_changes.py
```

Requiere Python 3, Ruby/Psych (lector YAML), Git y jq, disponibles en el runner
Ubuntu. No instala paquetes de tests. Crea repositorios Git temporales aislados,
nunca commits en el checkout. Cubre manifests/lock/toolchain/crates/apps/lab,
docs-only, cambios mixtos, borrados, áreas Java/frontend/Compose, cambios de CI,
fallback full y ejecución manual/schedule. Ejecuta el script real del gate con
resultados simulados y exige que Rust requerido termine en `success`, nunca
`skipped`; también comprueba errores/cancelaciones y contrato del workflow.
Las regresiones normativas verifican el comando `--require-complete` al final,
sin `continue-on-error` en job ni steps, su ejecución `always()`, ambas salidas
subidas con `always()` y la propagación de exit 2 al bloqueo de `CI Gate`.

## Concurrency

El workflow cancela runs obsoletos cuando llega un commit nuevo al mismo PR o a
la misma rama/evento:

```yaml
concurrency:
  group: ci-${{ github.workflow }}-${{ github.event_name }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
```

Así, un commit nuevo en el mismo PR cancela el run obsoleto, sin cancelar entre
sí runs independientes de `main` o `workflow_dispatch` (el grupo incluye el
evento).

## Caches

- **Maven**: `actions/setup-java@v4` con `cache: maven` (invalida con `pom.xml`).
- **pnpm**: `actions/setup-node@v4` con `cache: pnpm` y
  `cache-dependency-path: frontend/pnpm-lock.yaml`.
- **Playwright**: no se cachean los browsers; solo el CI de `main` instala
  Chromium, así que no compensa una cache frágil.

La versión de pnpm la fija `packageManager` en `frontend/package.json`
(`pnpm/action-setup@v4`); no se declara una versión distinta en el workflow.

## CI Gate

El job `CI Gate` depende de `changes`, `backend`, `rust`, `frontend`, `compose` y
`e2e`, se ejecuta siempre (`if: always()`) y **falla si algún job falló o se
canceló**. Exige `changes=success` y un `rust_run` válido (`true`/`false`): si
`rust_run=true`, exige **`rust=success`**, nunca `skipped`. Rust no requerido
puede estar `skipped`; se conserva el tratamiento previo de `skipped` para los
otros jobs condicionales.
Es el único _required status check_ recomendado en la protección de rama, lo que
evita depender de checks condicionales que a veces no existen.

## Validación manual

Desde la pestaña Actions, `workflow_dispatch` ejecuta la validación completa
(incluido el E2E) sobre un `target_sha` explícito (40 caracteres, desde `main`).
También puede lanzarse desde la CLI:

```bash
gh workflow run ci.yml --ref main -f target_sha=<commit-sha>
```

## Notas

- El servicio PostgreSQL es siempre `postgres:17` aislado y descartable
  (`hiqnet_pos_e2e`), nunca el volumen de desarrollo.
- Los artefactos de fallo E2E (traces, screenshots, video, logs) se suben solo
  con `if: failure()`, con 7 días de retención. La evidencia técnica y normativa
  Rust se sube con `if: always()`, también con 7 días de retención.
