# Incremento 0 / 0b — Foundation Rust y Migration Lab

**Fecha:** 2026-10-27. **Estado global: PARCIAL; merge de esquema BLOQUEADO.**
No hay datos reales, cutover, doble escritura, release, nueva UI ni retirada de
Java. Esta entrega es código ejecutable con pruebas, no el cierre de DDL-13.

El incremento 0b añadió PostgreSQL 17.11 nativo, origen Flyway V1–V25 real,
exportación determinista de las 37 tablas y AUTH-MIG sintético. Resultado: **41
comprobaciones técnicas verdes**, **1/30 casos normativos aprobados** (PAR-08,
exportador determinista) y gate
`--require-complete` con **exit 2**. El detalle por caso está en
[`rust-increment-0b-report.md`](rust-increment-0b-report.md).

## 1. Baseline y fuentes

`git fetch origin` confirmó `origin/dev` en
`ebf7a30afc342a31d837a85772c5e85d0bc38efb`, igual al SHA documental y al HEAD
inicial. No había cambios locales ni commits posteriores. Se creó
`feat/backend/rust-foundation-migration-lab` desde `origin/dev`, sin worktree.
Los cambios se conservan en la rama de trabajo. La publicación y el estado de CI
se registran en el informe 0b; un gate rojo no se presenta como PR aprobado.

La última CI remota observada del baseline era correcta:
[run 39180399982](https://github.com/HiQNet-Dev/point-of-sale/actions/runs/39180399982).
No representa la CI de estos archivos sin publicar.

| Fuente consultada              | Uso                                              | Límite                                          |
| ------------------------------ | ------------------------------------------------ | ----------------------------------------------- |
| F4–21                          | Autoridad de alcance Free y gates                | Diseño aprobado no equivale a tests ejecutados  |
| F4–00 / F4–06                  | Monolito modular y límites de workspace          | F4–21 prevalece sobre propuestas históricas     |
| F1–10 / F1–11 / F1–12 / F1–13  | Físico, constraints, paridad y harness           | Esquema completo y datasets aún pendientes      |
| F4–13 / F4–18 / F4–19          | Dinero exacto, SQLx y transición de credenciales | No inventar rounding ni política KDF definitiva |
| Índice maestro v3.0            | Localización del expediente                      | No reproduce documentación confidencial         |
| Java/Flyway/React del baseline | Comportamiento AS-IS                             | No se presume comportamiento solo por una guía  |

Referencias adicionales verificadas:

- [Índice maestro v3.0](https://docs.google.com/document/d/1DfIuSJCjXE1eBurbWXzeEewHRj7ceBOMCSaHsGLpJ0U/edit).
- [F4–00](https://docs.google.com/document/d/13IZVyKD1VKFOpWUMvzJz8NdEt_43HxGZvdoBlh6Goz8/edit).
- [F1–11](https://docs.google.com/document/d/1PCgbE95Qt3GjIfy78rPt-tCMjQ2Ed2GwBzEcTfa4HMU/edit).
- [F1–12](https://docs.google.com/document/d/16hyT4XALY3DrAc50RUTx-fESzMI0c5612zTKphHRieM/edit).
- [F4–13](https://docs.google.com/document/d/1vL-yFrwenxeqbAjuWY9m5zOMhlMGADfVTDUeWaOa4OM/edit).
- [F4–19](https://docs.google.com/document/d/1TTiJo3tHgbEdB043mLQKTayi8zEmP_WIk0vdshP2Bgk/edit).

Los documentos principales fueron accesibles mediante exportación pública. No
se dispone de herramienta autenticada para editar Drive; no se sincronizó ni se
publicó una copia del expediente. Referencias clicables en ADR-0020, la solicitud
de trabajo y la matriz AS-IS. Los complementos no leídos directamente no se
presentan como verificados: no se completó una auditoría editorial F3/F7 ni toda
la documentación F5/F6.

Se cargaron skills instalados de arquitectura, implementación incremental, Git,
TDD, seguridad, documentación, CI y revisión. Hay CLI Codex/OpenCode; no se leyó
su configuración de autenticación ni se asumieron perfiles. No se encontró skill
específico Rust en el proyecto. Se usaron subagentes reales en ámbitos disjuntos:
auditoría Java, fuentes/seguridad, core/runtime, harness, CI, regresión y revisión
independiente. No hay integración automática ni commits de subagentes.

## 2. Qué existe realmente

La [guía Rust](../../backend-rust/README.md) contiene diagrama, árbol, dependencias,
comandos y límites. Workspace edición 2024, toolchain 1.99.1, MSRV 1.99 y lockfile.

| Componente           | Implementado                                                                                                              | No implementado                                                                    |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `pos-core`           | Currency sintáctica, MoneyMinor `i64`, suma checked, conversión exacta con escala explícita                               | DecimalExact general, Quantity, rounding, costeo, casos de uso                     |
| `pos-storage-sqlite` | Archivo WAL/FULL, FK verificadas por conexión, timeout obligatorio, pool de un slot, migración Local embebida, checkpoint | Repositorios, ownership multiproceso, cifrado, backup de instalación               |
| `pos-local`          | Configuración tipada, composición explícita, tracing JSON cerrado, SIGINT/SIGTERM y cierre                                | Tauri, IPC, HTTP, auth, RBAC, comandos de negocio                                  |
| `migration-lab`      | Fixtures SQLite, PG17 nativo, origen Flyway real, 37 tablas exportadas, AUTH-MIG, verificación y evidencia honesta        | Import target/paridad, esquemas completos, ownership/rehost, sesiones persistentes |
| CI                   | Detección Rust y gates fmt/clippy/test/build/secrets/audit/DDL13, PG build y verificación de evidencia                    | Ejecución remota de esta rama (ver informe 0b)                                     |

Los errores públicos de configuración/storage no contienen rutas, SQL ni valores
externos. Se desactiva logging de sentencias y se filtra telemetría de dependencias.
La salida de diagnóstico SQL detallada solo pertenece al lab sintético.

## 3. Persistencia: diccionario de lo materializado

**No son las 82 fichas físicas completas** (la cifra editorial 83 no identifica
una tabla adicional). Las escalas, normalizaciones de códigos,
validación IANA/UTC y ownership se terminarán en su caso de uso. `UNIQUE(code)`
actual no equivale por sí solo a normalización/casefold aprobada. Ninguna tabla
habilita usuarios ni checkout. `STRICT` se aplica a tablas de dominio Local;
la tabla interna de tracking es responsabilidad de SQLx.

### Local — `0001_organization_branch.sql`

Fuente F1–10 §16.1–2; migra únicamente bases sintéticas.

| Campo                              | Tipo lógico   | Tipo físico | Clave      | NULL | Default | Constraint/índice                                                            |
| ---------------------------------- | ------------- | ----------- | ---------- | ---- | ------- | ---------------------------------------------------------------------------- |
| organization.id                    | UUID          | TEXT        | PK         | NO   | —       | Canónico minúsculo, 36 caracteres, guiones, hex y rechazo NUL                |
| organization.code                  | Código        | TEXT        | UK         | NO   | —       | UNIQUE; normalización de aplicación pendiente                                |
| organization.name                  | Texto         | TEXT        | —          | NO   | —       | Validación de aplicación pendiente                                           |
| organization.currency              | Código moneda | TEXT        | —          | NO   | —       | Tres ASCII A–Z, sin NUL; no catálogo ISO completo                            |
| organization.timezone              | IANA-TZ       | TEXT        | —          | NO   | —       | Validación IANA pendiente                                                    |
| organization.locale                | Locale        | TEXT        | —          | NO   | —       | Configuración explícita                                                      |
| organization.stock_negative_policy | Enum          | TEXT        | —          | NO   | —       | STRICT / ALLOW_NEGATIVE                                                      |
| organization.status                | Enum          | TEXT        | —          | NO   | ACTIVE  | ACTIVE / INACTIVE                                                            |
| organization.version               | Versión       | INTEGER     | —          | NO   | 0       | >=0                                                                          |
| organization.created_at            | UTC           | TEXT        | —          | NO   | —       | Normalización de aplicación pendiente                                        |
| organization.updated_at            | UTC           | TEXT        | —          | NO   | —       | Normalización de aplicación pendiente                                        |
| branch.id                          | UUID          | TEXT        | PK         | NO   | —       | Mismo CHECK UUID que organization                                            |
| branch.organization_id             | UUID          | TEXT        | FK         | NO   | —       | organization(id)                                                             |
| branch.code                        | Código        | TEXT        | UK parcial | NO   | —       | UNIQUE(organization_id,code), sin casefold implícito                         |
| branch.name                        | Texto         | TEXT        | —          | NO   | —       | Validación de aplicación pendiente                                           |
| branch.status                      | Enum          | TEXT        | —          | NO   | ACTIVE  | ACTIVE / INACTIVE                                                            |
| branch.timezone_override           | IANA-TZ       | TEXT        | —          | SÍ   | NULL    | Validación IANA pendiente                                                    |
| branch.version                     | Versión       | INTEGER     | —          | NO   | 0       | >=0                                                                          |
| branch.created_at                  | UTC           | TEXT        | —          | NO   | —       | UTC de aplicación pendiente                                                  |
| branch.updated_at                  | UTC           | TEXT        | —          | NO   | —       | UTC de aplicación pendiente                                                  |
| branch.(organization_id,id)        | Scope         | TEXT,TEXT   | UK         | NO   | —       | Referencia compuesta para hechos futuros; no límite de sucursales históricas |

### Central — `0001_operational_tenant.sql`

F1–10 §16.25. **No ejecutada en PostgreSQL en esta intervención.**

| Campo                    | Tipo lógico  | Tipo físico | Clave | NULL | Default | Constraint/índice                      |
| ------------------------ | ------------ | ----------- | ----- | ---- | ------- | -------------------------------------- |
| id                       | UUID         | UUID        | PK    | NO   | —       | Identidad entregada por llamador       |
| external_organization_id | UUID externo | UUID        | UK    | NO   | —       | UNIQUE; sin FK cross-DB                |
| status                   | Enum         | TEXT        | —     | NO   | ACTIVE  | ACTIVE/SUSPENDED/DECOMMISSIONED        |
| created_at               | UTC          | TIMESTAMPTZ | —     | NO   | now()   | Timestamp PostgreSQL                   |
| updated_at               | UTC          | TIMESTAMPTZ | —     | NO   | now()   | Sin trigger de actualización implícito |

### Control — `0001_customer.sql`

F1–10 §16.28. **No ejecutada en PostgreSQL en esta intervención.**

| Campo                | Tipo lógico | Tipo físico | Clave | NULL | Default | Constraint/índice                      |
| -------------------- | ----------- | ----------- | ----- | ---- | ------- | -------------------------------------- |
| id                   | UUID        | UUID        | PK    | NO   | —       | Identidad explícita                    |
| status               | Enum        | TEXT        | —     | NO   | ACTIVE  | ACTIVE/CLOSED                          |
| identity_verified_at | UTC         | TIMESTAMPTZ | —     | SÍ   | NULL    | Dato comercial                         |
| version              | Versión     | INTEGER     | —     | NO   | 0       | >=0                                    |
| created_at           | UTC         | TIMESTAMPTZ | —     | NO   | now()   | Timestamp PostgreSQL                   |
| updated_at           | UTC         | TIMESTAMPTZ | —     | NO   | now()   | Sin trigger de actualización implícito |

Local usa TEXT/INTEGER compatibles con STRICT; PostgreSQL conserva UUID y
TIMESTAMPTZ nativos. No se pretende equivalencia por sintaxis, no se añaden FK
entre motores ni se usan estos subconjuntos como prueba de aislamiento Central.
Los tres roots empiezan por 0001. Local, Central y Control se ejecutaron como
roots parciales en motores reales: SQLite y PostgreSQL 17 nativo. El lab usa
además historiales SQLx separados de fixtures (`lab_parent`, `lab_child`), nunca
los confunde con el esquema de negocio, con Flyway ni entre contextos. Que los
roots parciales instalen, actualicen y restauren **no** los convierte en
DDL13-01/02/03 completos.

## 4. Validación ejecutada y evidencia

| Comando / comprobación                                                          | Resultado observado                                                                         |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Preflight Git, fetch, revisión de delta                                         | Checkout limpio inicial, sin delta del SHA de referencia                                    |
| `cargo fmt --all -- --check`                                                    | Correcto                                                                                    |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | Correcto                                                                                    |
| `cargo test --locked --workspace --all-features`                                | 59 pruebas correctas, 0 fallidas y 0 ignoradas                                              |
| `cargo build --locked --workspace --all-features`                               | Correcto                                                                                    |
| Lab normal con `--postgres-bin` y `--java-source`                               | Exit 0; **41/41 checks técnicos**, 1/30 normativos (PAR-08)                                 |
| Lab `--require-complete` con PG y source                                        | **Exit 2**, gate normativo bloqueado, comportamiento esperado                               |
| PostgreSQL 17.11 nativo                                                         | 19 checks reales, sin Docker ni TCP; cleanup verificado                                     |
| Origen Flyway V1–V25                                                            | 25 migraciones, 37/37 tablas, dos instalaciones con digest funcional idéntico               |
| AUTH-MIG-01..08                                                                 | 8/8 técnicos; política `lab-argon2-v1` no aprobada comercialmente                           |
| `verify_evidence.py`                                                            | Valida 41 técnicos, 30 bloqueados, Money/Quantity y ausencia de hashes en evidencia pública |
| Ejecución SQLite manual con `unshare --user --map-root-user --net`              | 11 checks técnicos; solo loopback y sin rutas IPv4; no aprueba DDL13-10 completo            |
| `python3 .github/scripts/test_ci_changes.py`                                    | 21 tests correctos, incluidos gate de exit2 y prohibición de falso skip                     |
| actionlint 1.7.7                                                                | Correcto, sin ShellCheck; sintaxis Bash comprobada aparte                                   |
| `cargo audit --file Cargo.lock --json` con 0.22.2                               | 0 vulnerabilidades, sin warnings al consultar RustSec                                       |
| Gitleaks 8.24.3, SHA-256 verificado, `--redact`                                 | Sin leaks en fuentes (target generado excluido); no es garantía universal                   |
| Java `./mvnw -B -ntp clean verify`                                              | **Fallo exit1**: compila, Testcontainers no puede acceder a Docker                          |
| React `pnpm lint && pnpm typecheck && pnpm test && pnpm build`                  | Correcto; Vitest 194/194 en 19 archivos                                                     |
| `docker compose config --quiet`                                                 | Correcto; no implica daemon operativo                                                       |
| Playwright                                                                      | 8 escenarios descubiertos; no ejecutados                                                    |
| `git diff --check`                                                              | Correcto; inspección final requerida también sobre archivos nuevos                          |
| Diff de backend/frontend/Compose/Flyway                                         | Sin cambios versionados                                                                     |

La suite Rust incluye dinero exacto/overflow/moneda, configuración, telemetría,
cierre incluso ante error, SQLite real, SIGINT/SIGTERM del ejecutable, reporting,
fuentes, salida bloqueada y protección de artefactos. Se observaron tests rojos
antes de implementar storage y antes de corregir los dos defectos de revisión:
close de SQLx invocado antes del checkpoint y sufijos NUL en CHECKs SQLite.
Ambos tienen regresiones automatizadas. Una prueba añadida después valida los
30 identificadores normativos exactos, sin aprobarlos por tener un nombre.

Java: Surefire recién ejecutado reporta **843 casos: 111 correctos, 732 errores,
0 fallos de aserción y 0 omitidos**. Los errores provienen de Docker/contextos
fallidos, no prueban 732 regresiones de negocio. `verify` no terminó. Los 860
casos inventariados o de reportes previos en la matriz AS-IS no se presentan como
860 tests ejecutados correctamente aquí. No se debilitaron suites ni filtros.
React emite advertencia existente por chunk principal de 764,70 kB (>500 kB).

El primer intento con cargo-audit 0.21.2 falló por no interpretar CVSS 4.0 de
RustSec; se actualizó el auditor a 0.22.2, no se ignoró el advisory ni se rebajó
la base. Las herramientas auxiliares y descargas requirieron red; el runtime
local no tiene dependencia de Control Plane ni licencias.

### Matriz normativa: sin verde artificial

| Casos        | Estado  | Qué falta                                                                             |
| ------------ | ------- | ------------------------------------------------------------------------------------- |
| DDL13-01     | BLOCKED | Esquema Local completo, no solo organization/branch                                   |
| DDL13-02..03 | BLOCKED | Esquemas Central/Control completos, ejecución PG17 aislada                            |
| DDL13-04..09 | BLOCKED | Cobertura normativa por todos los contextos; las fixtures SQLite no la sustituyen     |
| DDL13-10     | NOT_RUN | Instalación/upgrade offline del esquema Local completo; wrapper técnico no basta      |
| DDL13-11     | BLOCKED | Aislamiento real de los tres historiales en sus motores                               |
| PAR-01..09   | BLOCKED | Source Java/Flyway vivo, dataset37, exportador/importador y comparación semántica     |
| AS-01..10    | BLOCKED | Assertions de negocio trazables, incluidas credenciales y recuperación de instalación |

41 checks técnicos pasan: 11 SQLite, 19 PostgreSQL, 8 AUTH-MIG y 3 de origen/
reconstrucción. Incluyen FK por conexión, STRICT, upgrade, checksum, rollback,
rerun, backup/restore, roles PG, fronteras entre bases, credenciales sintéticas,
37/37 tablas source, Money exacto y conciliación de stock. **No son DDL13/PAR/AS
aprobados.** JUnit representa los 30 casos pendientes con `skipped` acompañado
del estado BLOCKED/NOT_RUN; no son tests Rust ignorados ni éxitos de gate. CI
propaga exit2.

Artefactos finales generados en `backend-rust/target/increment0b-evidence-6/` y
salida estricta en `target/increment0b-gate-5/`. Las ejecuciones anteriores se
conservan separadas; las siguientes deben elegir otro
directorio nuevo. Los artefactos contienen baseline/migration manifests, JSON y
JUnit de paridad/técnicos, counts, integrity, money-vectors, stock-reconciliation,
idempotency, auth-transition, security-report, schema-snapshots, failure-logs,
backup-fixtures/restoration-report, digests técnicos y run-summary. Los `.sha256`
normativos sin ejecución contienen JSON BLOCKED **sin un digest inventado**.
Los backups son SQLite sintético temporal, no datos de cliente. CI los publica
como artifacts, no como una certificación. `target/` permanece ignorado.

El run manual aislado queda en `target/migration-offline-evidence/`, con
`offline-execution.json`; el runner no certifica por sí solo su namespace. Esa
observación puntual tampoco mide operación offline sostenida, ownership o backup
de una instalación completa. Hay auditoría RustSec en `target/cargo-audit.json`
y scanner redactado en `target/gitleaks-report.json`.

## 5. Riesgos, pendientes y gate de continuación

1. **No se terminó incremento0b.** El físico sigue parcial: cuatro tablas de
   negocio, no las 82 fichas. DDL13 completo, import target, RLS y paridad
   semántica no están implementados.
2. F1–10 declara 83 tablas, pero solo contiene **82 fichas** (59 Local, 9 Central,
   14 Control) y cinco dependencias sin ficha (`price_book`, `tax_profile`,
   `CostingPolicy`, `ExpenseCategory`, `plan`). No se inventaron tablas ni campos.
3. `unit_cost`: Java admite NULL en ORDERED y RECEIVED; F1–10 exige
   `unit_cost_exact` y `line_total_exact` NOT NULL. **No se convierte NULL en
   cero.** [ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md)
   queda propuesto; el import de compras está bloqueado.
4. Docker local sigue denegado (`root:docker`, usuario sin grupo). La alternativa
   válida fue PostgreSQL 17.11 nativo aislado, no H2 ni mocks. La suite Java con
   Testcontainers no se ejecutó: no se declara aprobada.
5. F4–19 acepta BCrypt→Argon2id, pero la política m/t/p no está aprobada para
   producto. AUTH-MIG es una simulación en memoria; faltan atomicidad persistente,
   sesiones revocables y benchmark en hardware objetivo. No se habilitaron
   usuarios reales ni se migraron JWT/refresh tokens.
6. Faltan escalas/rangos definitivos de operandos, rounding concreto, Quantity,
   costeo, snapshots, outbox y RBAC persistente. `MoneyMinor` no redondea.
7. No hay cobertura porcentual ni matriz de features opcionales: no se declara un
   umbral cumplido sin contrato. La suite no demuestra paridad comercial.

## 6. Rollback y siguiente incremento exacto

Java conserva autoridad y sus archivos no cambian. Para detener la foundation,
terminar su proceso de prueba y descartar únicamente sus artefactos sintéticos
propios. No hacer import, dual-write ni down migration sobre clientes. No existe
rollback de cutover porque no se ejecutó cutover.

**Siguiente incremento: 0c — resolver el contrato físico y completar DDL-13.**
Requiere aprobación de ADR-0021 (conteo 82/83, cinco dependencias y semántica
`unit_cost`/totales), materializar el físico completo sin inventar, import target
idempotente y comparación semántica. Aceptación: DDL13-01..11, PAR-01..09 y
AS-01..10 verdes con evidencia exacta; `--require-complete` exit0, regresión Java
con PostgreSQL real, CI remota verificable y revisión independiente. Solo después
avanzar a Organization/Branch/Users/Auth/RBAC productivo.
