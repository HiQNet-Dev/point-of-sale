# Informe — Incremento 0b: foundation, Migration Lab y paridad

**Fecha:** 2026-10-27. **Estado global: PARCIAL.** La foundation y el laboratorio
son ejecutables y verificables. El cierre normativo de DDL-13 sigue **BLOQUEADO**
por el contrato físico incompleto y la discrepancia `unit_cost`. No hay datos
reales, cutover, release, nueva UI ni retirada de Java.

## 1. Baseline y preservación

- Rama: `feat/backend/rust-foundation-migration-lab`.
- Baseline: `origin/dev@ebf7a30afc342a31d837a85772c5e85d0bc38efb`.
- No se usaron `reset --hard`, `clean -fd` ni `checkout -- .`.
- Flyway V1–V25 no cambió. Backend Java, frontend y Compose no se modificaron.
- Los artefactos previos de Incremento 0 se conservaron; las ejecuciones nuevas
  usaron directorios nuevos.

## 2. PostgreSQL y Java

El socket Docker local pertenece a `root:docker` y el usuario no está en el grupo;
`docker.service` está inactivo. No se cambiaron permisos ni grupos. La alternativa
aislada fue **PostgreSQL 17.11 compilado desde fuente oficial** con SHA-256
`dd27f2b3c59e73ed14aa3324901242bf69a032a6347805f274e6260322d42979`, socket UNIX
privado `0700`, `listen_addresses=''`, sin TCP y sin `DATABASE_URL`.

Resultado nativo: **19/19 checks técnicos PostgreSQL** (clean roots parciales,
upgrade, checksum, rollback, rerun, roles, fronteras y dump/restore). No se
aprobó DDL13-02/03 porque Central y Control siguen siendo subconjuntos de una
tabla cada uno.

La suite Java/Testcontainers **no se ejecutó** por el socket denegado. El origen
Flyway V1–V25 sí se ejecutó con JDBC/Java source-file sobre PostgreSQL 17.11,
pero eso **no** equivale a `./mvnw clean verify` ni a E2E. No se declara Java
verde.

## 3. Origen Java/Flyway y paridad

Dos instalaciones limpias aplicaron **25 migraciones Flyway reales** y un seed
sintético. Resultado verificado:

- **37/37 tablas pobladas**; 166 filas source, 165 exportadas (una sesión
  deliberadamente invalidada).
- Bundle canónico v1: UUID sin regenerar, `NUMERIC` como string exacto desde
  `BigDecimal`, `NULL` preservado, timestamps UTC, JSONB como texto.
- Dos instalaciones independientes y dos exportaciones por snapshot con
  **digest funcional idéntico**.
- `refresh_sessions`: `INVALIDATE`; no se exportan identidades ni token hashes.
- `app_metadata`: `REFERENCE_ONLY`.

## 4. Money, Quantity y stock observados

- `900719925474.1234` se convierte exactamente a `9007199254741234` con escala 4.
- El mismo valor a escala 2 se **rechaza**; no se redondea.
- Venta `12.3456` a escala 2 se rechaza; a escala 4 es `123456`.
- `unit_cost` source: un `NULL` y un `0.0000`; ambos se conservan distintos.
- Cantidades de stock se suman con enteros `i128` a escala 6. La cadena
  `resulting_quantity` es válida y el balance `11.000000` coincide con el ledger.
- Esto es evidencia técnica, **no** AS-04/AS-05 normativos: falta import target.

## 5. Autenticación sintética

`AUTH-MIG-01..08` pasaron 8/8 sobre la máquina de estados en memoria:

- BCrypt legado válido → Argon2id, versión incrementada y sesión local sintética.
- Password incorrecta → sin rehash, contador incrementado.
- Argon2id vigente → login sin rehash innecesario; policy antigua → rehash.
- Lock activo e INACTIVE → rechazo sin rehash ni sesión.
- Hash corrupto/policy desconocida → `PASSWORD_RESET_REQUIRED`.
- Handles/sesiones Java → rechazados; cero importación de refresh/JWT.

Política `lab-argon2-v1`: Argon2id, m=19456 KiB, t=2, p=1, 16-byte salt,
32-byte output. **No está aprobada comercialmente**. En debug integrado el
benchmark midió ≈398 ms p50; el harness release separado midió ≈15,5 ms.
Falta benchmark en hardware objetivo y presupuesto UX/RAM.

## 6. Matriz de los 30 casos normativos

| ID       | Estado     | Evidencia                                                                        | Causa / alcance                                                                         |
| -------- | ---------- | -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| DDL13-01 | BLOCKED    | `parity-report.json`; `schema-snapshots/storage-local-subset.json`               | Local solo tiene `organization`/`branch`; faltan las 59 fichas Local.                   |
| DDL13-02 | BLOCKED    | `postgres-technical-report.json`; `schema-snapshots/postgres-central-clean.json` | PostgreSQL 17 real ejecutó el root parcial; Central completo no materializado.          |
| DDL13-03 | BLOCKED    | `postgres-technical-report.json`; `schema-snapshots/postgres-control-clean.json` | Control completo no materializado; root parcial ejecutado.                              |
| DDL13-04 | BLOCKED    | `postgres_*_fixture_upgrade...`; `upgrade_preserves_data`                        | Upgrade N-1→N solo sobre fixtures/roots parciales; no esquema completo.                 |
| DDL13-05 | BLOCKED    | `postgres_*_checksum...`; `checksum_mismatch`                                    | Checksum se probó en SQLite y PG17 con fixtures, no en los tres esquemas completos.     |
| DDL13-06 | BLOCKED    | `failure-logs/expected-transactional-failure.txt`                                | Fallo transaccional SQLite verificado, pero no dentro del esquema Local completo.       |
| DDL13-07 | BLOCKED    | `postgres-*-expected-ddl-failure.json`                                           | Rollback PG verificado en fixture; falta upgrade completo de Central/Control.           |
| DDL13-08 | BLOCKED    | `backup-fixtures/postgres-*-restore.json`; `restore.sha256`                      | Restore idéntico sintético; no es restauración de una instalación/esquema completo.     |
| DDL13-09 | BLOCKED    | `rerun_history_immutable`; `postgres_*_rerun_exact_history`                      | Rerun idempotente verificado en roots parciales, no en los tres esquemas completos.     |
| DDL13-10 | NOT_RUN    | `migration-offline-evidence/offline-execution.json`                              | Solo se ejecutó SQLite bajo namespace sin red; el esquema Local completo no existe.     |
| DDL13-11 | BLOCKED    | `postgres_database_boundaries`; historiales separados                            | Historiales aislados técnicamente; falta materialización completa de los tres roots.    |
| PAR-01   | BLOCKED    | `source-report.json`; `counts.json`                                              | 37/37 source pobladas, pero no hay import/acción target para cada tabla.                |
| PAR-02   | BLOCKED    | seed source 2 org/3 branch; `counts.json`                                        | Identidades source preservadas; no hay rechazos cross-tenant en target.                 |
| PAR-03   | BLOCKED    | `money-vectors.json`                                                             | Vectores exactos source/core; falta comparación contra import target completo.          |
| PAR-04   | BLOCKED    | `stock-reconciliation.json`                                                      | Ledger source conciliado a escala 6; falta target y reconciliación completa.            |
| PAR-05   | BLOCKED    | `auth-transition.json`                                                           | Auth sintética en memoria; falta importador de credenciales a Local.                    |
| PAR-06   | BLOCKED    | source `sales`, snapshots y modifier; `counts.json`                              | Venta/snapshots source presentes; falta target e igualdad semántica completa.           |
| PAR-07   | BLOCKED    | source PO/receipts; ADR-0021                                                     | Recepciones source presentes; `unit_cost NULL` impide import fiel.                      |
| PAR-08   | **PASSED** | `source-report.json` (`PAR-08`), `source-manifest.json`                          | Dos instalaciones limpias V1–V25, 37/37 y digest funcional idéntico. No importa target. |
| PAR-09   | BLOCKED    | source script; target import blocked                                             | No se ejecutó import target repetido; no se puede probar no duplicación.                |
| AS-01    | BLOCKED    | `counts.json`                                                                    | Conteos source-only; falta comparación source↔target.                                   |
| AS-02    | BLOCKED    | `source-manifest.json`                                                           | UUID source preservados; falta digest de IDs target.                                    |
| AS-03    | BLOCKED    | PostgreSQL boundaries/privilegios                                                | Fronteras entre bases verificadas; falta ownership/FK/RLS del target completo.          |
| AS-04    | BLOCKED    | `money-vectors.json`                                                             | Exactitud observada; falta conversión/import target y redondeo aprobado.                |
| AS-05    | BLOCKED    | `stock-reconciliation.json`                                                      | Conciliación source exacta; falta ledger target completo.                               |
| AS-06    | BLOCKED    | source snapshots                                                                 | Snapshots source presentes; falta comparación target y digest normativo.                |
| AS-07    | BLOCKED    | `immutability-tests.xml`                                                         | Solo historial SQLx; falta probar UPDATE/DELETE sobre hechos completos.                 |
| AS-08    | BLOCKED    | `idempotency.json`                                                               | Rerun SQLx no acredita idempotencia de checkout/recepción target.                       |
| AS-09    | BLOCKED    | `auth-transition.json`                                                           | 8/8 técnicos; falta transacción persistente, sesiones y benchmark aprobado.             |
| AS-10    | BLOCKED    | restore sintético                                                                | Restore técnico coincide; falta restore de instalación/esquema completo.                |

**Resumen:** 1 PASSED (PAR-08), 28 BLOCKED, 1 NOT_RUN. `--require-complete`
devuelve **exit 2** y CI no puede aprobar el merge.

## 7. Pruebas y artefactos

Comandos ejecutados desde `backend-rust/`:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo build --locked --workspace --all-features
sh tools/migration-lab/scripts/build_postgres.sh
cargo run --locked -p migration-lab -- --output target/increment0b-evidence-6 \
  --postgres-bin target/pg17-build/install/bin --java-source
python3 tools/migration-lab/scripts/verify_evidence.py \
  --evidence target/increment0b-evidence-6 --expect-exit 0
cargo run --locked -p migration-lab -- --output target/increment0b-gate-5 \
  --postgres-bin target/pg17-build/install/bin --java-source --require-complete
python3 tools/migration-lab/scripts/verify_evidence.py \
  --evidence target/increment0b-gate-5 --expect-exit 2
```

Resultados: **59 pruebas Rust correctas**, 0 fallidas/ignoradas; **41/41 checks
técnicos**; RustSec **0 vulnerabilidades/sin warnings**; Gitleaks sin leaks en
las fuentes (se excluyó `target/` generado, que contiene falsos positivos del
propio código fuente de PostgreSQL); actionlint correcto; **21 tests de CI**
correctos.

Evidencia: `backend-rust/target/increment0b-evidence-6/` y
`backend-rust/target/increment0b-gate-5/`. Incluye manifests, JUnit, snapshots,
logs de fallos esperados, dumps sintéticos, `source-report.json`,
`source-manifest.json`, `money-vectors.json`, `stock-reconciliation.json`,
`auth-transition.json`, `security-report.json` y `run-summary.md`. Los bundles
privados con hashes sintéticos no se copian. Los `.sha256` normativos pendientes
contienen JSON BLOCKED sin digest inventado.

## 8. Git y CI

- Commits locales: `4236fab` (foundation), `7de03ed` (migration lab),
  `c995bf8` (CI) y `6567f87` (evidencia/publicación), sobre
  `origin/dev@ebf7a30`.
- `git push -u origin feat/backend/rust-foundation-migration-lab` fue **bloqueado
  por la regla de seguridad del terminal** antes de contactar GitHub. No se creó
  PR remoto ni se ejecutó CI remota de estos cambios. No se eludió la regla.
- El árbol quedó limpio tras los commits; Flyway/Java/React/Compose sin cambios.
- La CI definida ejecutaría PG 17 nativo y el gate obligatorio; se espera rojo por
  los 29 casos pendientes. Sin push no se presenta un run remoto ficticio.

## 9. Bloqueos materiales y siguiente paso

1. **82 vs 83**: F1–10 contiene 82 fichas tabulares; la tabla HTML vacía explica
   la cifra editorial, no una entidad adicional. No se inventó una tabla 83.
2. **Cinco dependencias sin ficha**: `price_book`, `tax_profile`, `CostingPolicy`,
   `ExpenseCategory`, `plan`. No se crearon por deducción.
3. **`unit_cost`**: Java permite NULL en ORDERED/RECEIVED; F1–10 exige
   `unit_cost_exact`/`line_total_exact` NOT NULL. No se convirtió NULL en cero.
   [ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md)
   queda propuesto y el import de compras bloqueado.
4. **Target import**: sin resolver el contrato físico no se importa, no se
   compara source/target y no se aprueban PAR/AS restantes.
5. **Java/E2E**: Docker local denegado; no se declara la suite Java aprobada.

**Siguiente incremento: 0c** — aprobar/ajustar el contrato físico, materializar
las fichas completas, implementar import target idempotente y ejecutar
DDL13-01..11/PAR-01..09/AS-01..10. Solo entonces `--require-complete` puede
devolver 0. No se inicia Organization/Users/Auth productivo antes de ese cierre.
