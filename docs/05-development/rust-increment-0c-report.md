# Informe — Incremento 0c: resolución propuesta y evidencia real

**Fecha: 2026-10-28. Estado global: PARCIAL; cierre material BLOQUEADO.**
ADR-0021 permanece **PROPOSED**, sin aceptación del titular. Se preparó un modelo
revisable y control de cambios, no un esquema completo aprobado. No hay datos
reales, cutover, doble escritura, endpoints nuevos, Pro ni retirada de Java.

> **Seguimiento PRE0D-1:** después del estado local relatado aquí,
> `43d9909` y `329c098` quedaron en `origin/dev`. La CI real del último es
> [40775256216](https://github.com/HiQNet-Dev/point-of-sale/actions/runs/40775256216),
> fallida solo en gate normativo Rust y CI Gate; Java/React/Compose correctos,
> E2E omitido. Este informe conserva la cronología de 0c, no certifica ausencia
> de publicación actual. Estado y propuesta final: [paquete PRE0D-1](rust-0d-approval-package.md).

## 1. GitHub, seis commits y baseline

La [reconciliación Git/CI](rust-increment-0c-git.md) documenta lecturas API,
historia, seis commits/padres y protección. Resumen:

- Base remota confirmada tras fetch: `origin/dev@6a15ee6a21ef80c17ab7276b337946cbcc4a4d37`.
- Baseline funcional Java: `ebf7a30afc342a31d837a85772c5e85d0bc38efb`, sin cambios.
- Rama nueva: `feat/backend/rust-physical-resolution`, nacida del ref reconciliado.
- Los seis commits ya estaban en dev, sin PR asociado observado. No se repitieron.
- [CI 40641592100](https://github.com/HiQNet-Dev/point-of-sale/actions/runs/40641592100):
  Java/React/Compose correctos, Rust falla solo en `--require-complete`, CI Gate
  propaga fallo, E2E omitido. No es CI de los cambios 0c.
- Protección dev: respuesta `Branch not protected`; rulesets y reglas activas
  aplicables vacíos. Ninguna configuración remota modificada.
- Commit técnico 0c: `43d9909da7d295fc56b7f3a7ad4907fbb04080ef`
  (`test(migration): verify snapshot digests and reject incomplete evidence`).
  El commit documental posterior se obtiene en `git log`; no se fuerza una cadena
  autorreferente de commits para escribir su propio SHA en este archivo.
- No push, PR ni CI nuevos de 0c en esta intervención. La integración existente
  no se revierte: las alternativas A/B requieren decisión del titular.

## 2. Conflictos y propuesta de arquitectura

| Conflicto | Estado | Entregable / decisión pendiente |
| --- | --- | --- |
| PHY-PAR-01 | BLOQUEADO para cierre editorial | 82 fichas (59/9/14) comprobadas, 83 objetos HTML incluido uno vacío. Causa editorial es hipótesis; autor debe confirmar errata o entidad ausente. |
| PHY-PAR-02 | BLOQUEADO para DDL | Cinco diccionarios propuestos, con fuentes de negocio, scope, cardinalidad, campos/tipos físicos, claves, NULL/defaults, constraints, índices, versionado y tests; resolver preguntas Q. |
| PHY-PAR-03 | BLOQUEADO | Costo pactado nullable y total desconocido juntos. NULL no es cero; no imponer obligación retrospectiva. |
| PHY-PAR-04 | BLOQUEADO | Receipt Java no tiene costo propio. B+D propone evidencia/valoración pendiente y regularización append-only; no fabricar asientos ni promedio confirmado. |
| PHY-PAR-05 | BLOQUEADO | Mapping 47/47 columnas de compras y referencias de ledger; destinos ausentes y hechos nunca registrados identificados explícitamente. |

**Recomendación:** B+D de [ADR-0021 revisado](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md):
precio pactado, costo efectivo y valoración son hechos distintos. Preservar legado
sin evidencia económica; exigir base documentada para nueva recepción Rust;
regularizar sin reescribir snapshots ni duplicar cantidad. La cadena económica
requiere orden causal acreditado, no fecha+UUID arbitrarios. Si la reconstrucción
es ambigua, no emitir valoración confirmada. No se adopta esta solución todavía.

Anexos:

- [Diccionario físico faltante](rust-missing-physical-proposal.md): `price_book`,
  `tax_profile`, `CostingPolicy`→`costing_policy` propuesto, `ExpenseCategory`→
  `expense_category` propuesto y `plan`; distingue N/C/P/Q y hashes de fuentes.
- [Compras Java→Rust](rust-purchase-parity-contract.md): 17+12+9+9 columnas,
  estados, ledger, límites SQL frente al servicio y casos ausentes de origen.
  Registra la desviación de replay duplicado Java, sin perpetuarla automáticamente.
- [CC-0021](rust-adr0021-change-control.md): cinco solicitudes y decisiones D01..07,
  impacto F1–10/F1–11/F1–12/F4–13/F4–15/F4–21, aceptación y rollback. No se
  editaron fuentes canónicas de Drive ni se las declaró actualizadas.

## 3. Cambios técnicos seguros y alcance real del físico

El verificador antes ignoraba `schema-snapshots/technical-digests.json`.
Ahora recalcula SHA-256 sobre bytes y exige los siete snapshots integrados,
rechazando faltantes, truncados, duplicados, rutas inesperadas, digests inválidos
y symlinks. Ocho tests sintéticos verifican fallos y controles válidos. Eso prueba
integridad relativa al manifest, **no autenticidad**, firma o aprobación normativa.

La CI ejecuta esos tests como step bloqueante y su test de contrato exige el
comando. Se conserva `--require-complete`, el rechazo de Rust omitido cuando es
obligatorio y los jobs Java/React/Compose/E2E existentes.

| Contexto | Tablas de producto realmente materializadas | Migration preservada | No confundir con |
| --- | --- | --- | --- |
| Local SQLite STRICT | **2**: organization, branch | `local/0001_organization_branch.sql` | 59 fichas existentes del documento, más propuestas por resolver. |
| Central PostgreSQL | **1**: operational_tenant | `central/0001_operational_tenant.sql` | 9 fichas documentadas; no sync Pro activo. |
| Control PostgreSQL | **1**: customer | `control/0001_customer.sql` | 14 fichas documentadas; no billing activo. |

Los conteos excluyen `_sqlx_migrations`, fixtures lab y tablas source. **Cero DDL
nuevo en 0c**; `Cargo.lock`, dependencias, Java/Flyway, frontend y Compose sin
cambios. No se crea tabla 83 ni padre ficticio. El laboratorio ejecuta raíces
parciales y fixtures N-1/N, no las confunde con esquema normativo completo.

## 4. Validación exacta

Desde `backend-rust/`, salvo indicación:

| Comando / comprobación | Resultado observado |
| --- | --- |
| `cargo fmt --all -- --check` | exit 0. |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | exit 0. |
| `cargo test --locked --workspace --all-features` | exit 0; **59 correctas, 0 fallidas, 0 ignoradas**. Log `target/increment0c-cargo-tests.log`. |
| `cargo build --locked --workspace --all-features` | exit 0. |
| `python3 -B tools/migration-lab/scripts/test_verify_evidence.py` | exit 0; **8 tests**, 0 errores/fallos/omitidos. |
| `python3 .github/scripts/test_ci_changes.py` (raíz repo) | Red inicial: exit 1 por comando de tests ausente; tras añadir step: exit 0, **21 correctos**. |
| `cargo run --locked -p migration-lab -- --output target/increment0c-evidence-1 --postgres-bin target/pg17-build/install/bin --java-source` | exit 0 técnico; **41/41**, 1/30 normativos, no cierre. |
| Mismo CLI, `--output target/increment0c-gate-2 --require-complete` | **exit 2**, 41/41 técnicos, 1 aprobado, 29 pendientes. También `increment0c-gate-1` devolvió 2. |
| `python3 tools/migration-lab/scripts/verify_evidence.py --evidence target/increment0c-gate-2 --expect-exit 2` | exit 0; verifica el **bloqueo**, no convierte el exit 2 en aprobación. |
| Verificador sobre `increment0c-evidence-1`, gate-1 y ambas salidas de artefacto remoto descargado | exit 0 con expectativa 0/2 respectiva; los siete digests técnicos de cada salida coinciden. |
| `cargo audit --file Cargo.lock --json` | exit 0; cargo-audit 0.22.2, **0 vulnerabilidades, warnings vacíos**; `target/cargo-audit-0c.json`. |
| Gitleaks sobre `docs`, `.github` y scripts del laboratorio, con `--redact` | Tres comandos exit 0; **sin leaks**, reportes `target/gitleaks-0c-*.json`. No se escanea `target/` fuente PG generada. |
| `git diff --check`, enlaces locales de anexos, diff de componentes preservados | Correctos; backend/React/Compose/lock/migrations sin cambios. |
| Suite Java local / frontend local / E2E local nuevos | **NO EJECUTADOS en 0c**: no cambió código Java/React; evidencia remota del SHA base indicada por separado. Docker local sigue permiso denegado, sin cambiar grupos/socket. |
| Actionlint nuevo | NO EJECUTADO: binario no disponible en PATH; el parser YAML y contrato del workflow se validaron mediante los 21 tests existentes. |

PostgreSQL **17.11 real**, socket UNIX privado, sin TCP: **19 checks** dentro de
los 41; checksum, rollback y dump/restore técnicos de ambos contextos correctos.
El origen ejecutó V1–V25 dos veces, **37/37 tablas pobladas**, digest funcional
idéntico. Limpieza observada: pools cerrados, proceso esperado y cluster temporal
eliminado. No es aprobación de la suite Java/Testcontainers local.

## 5. Los 30 escenarios normativos

Fuente machine-readable nueva: `backend-rust/target/increment0c-gate-2/parity-report.json`.
**T = evidencia técnica de subconjunto, no conformidad normativa**. Toda fila
pendiente conserva su estado real, aunque exista prueba técnica parecida.

| ID | Estado real | Evidencia / causa de bloqueo | Criterio pendiente de salida |
| --- | --- | --- | --- |
| DDL13-01 | BLOCKED | T Local parcial; 2 tablas, no físico completo | Clean completo, constraints/FK/history del baseline aprobado. |
| DDL13-02 | BLOCKED | T PG Central parcial | Clean 9 fichas reconciliadas, aislamiento/roles. |
| DDL13-03 | BLOCKED | T PG Control parcial | Clean físico Control completo, padres aprobados. |
| DDL13-04 | BLOCKED | T upgrades de fixtures | Snapshot N-1 real y N completo por tres contextos, digests/smoke. |
| DDL13-05 | BLOCKED | T checksums SQLite/PG | Alteración controlada de migrations del baseline completo, fallo sin continuación. |
| DDL13-06 | BLOCKED | T fallo transaccional SQLite | Upgrade fallido sobre esquema completo, inspección DB/history. |
| DDL13-07 | BLOCKED | T rollback PG por dos contextos | DDL inválido sobre targets completos; ningún success falso. |
| DDL13-08 | BLOCKED | T backup/dump/restore exactos de fixtures | Restore N-1 completo tras fallo, digests y reapertura. |
| DDL13-09 | BLOCKED | T rerun/history | Idempotencia del conjunto completo, datos/history iguales. |
| DDL13-10 | NOT_RUN | `offline_network_denied.status=NOT_RUN` | Binario embebido y clean/upgrade completos bajo red realmente denegada. Cargo offline no basta. |
| DDL13-11 | BLOCKED | T roots/historias independientes | Aislamiento de los tres esquemas completos con mismos números. |
| PAR-01 | BLOCKED | Origen 37/37, sin target import | Cero filas/tablas sin acción explícita y comprobada. |
| PAR-02 | BLOCKED | Sin comparación source/target | 2 org/3 branch, IDs y rechazos cross-scope. |
| PAR-03 | BLOCKED | Vectores exactos source/core, sin import | Money límites/0/4 decimales/no representables, sin pérdida. |
| PAR-04 | BLOCKED | Conciliación source técnica, sin target | Quantity 6 decimales, ledger/balances iguales por scope. |
| PAR-05 | BLOCKED | AUTH sintético técnico, sin credenciales persistidas target | ACTIVE/INACTIVE, locks, hash/policy y rechazo seguro; hardware/policy aprobados. |
| PAR-06 | BLOCKED | Sin import ventas/snapshots | Venta, modifiers/recipes, totales/consumos y snapshots exactos. |
| PAR-07 | BLOCKED | PHY-PAR-03..05 abiertos | PO/receipts, costos ausentes explícitos, estados/ledger/idempotencia. |
| PAR-08 | **PASSED** | Dos instalaciones V1–V25, 37/37 y digest funcional igual; `source_exporter_determinism` | Mantener regresión; no equivale a paridad target. |
| PAR-09 | BLOCKED | No existe import target | Dos imports del mismo bundle, no-op/fail-fast documentado, 0 duplicados. |
| AS-01 | BLOCKED | counts source-only | Conteos source↔target con acciones explicadas. |
| AS-02 | BLOCKED | Sin digest ID target | UUID/keys exactos, digest ordenado comparable. |
| AS-03 | BLOCKED | FK subset/source, sin ownership target completo | Sin huérfanos ni cross-scope por catálogos/intentos reales. |
| AS-04 | BLOCKED | Money solo técnico | Comparación exacta importada, sin redondeo silencioso. |
| AS-05 | BLOCKED | Cadena quantity source, no target | Quantity y ledger reconciliados completos. |
| AS-06 | BLOCKED | Mapping preparado, no import | Snapshots canónicos iguales aun tras cambiar catálogo. |
| AS-07 | BLOCKED | No hechos target completos | UPDATE/DELETE prohibidos fallan con rol/motor previsto. |
| AS-08 | BLOCKED | Sin comandos/import target | Replay e import no duplican; conflicto no sobrescribe historia. |
| AS-09 | BLOCKED | 8 AUTH-MIG técnicos sin policy de producto ni target completo | Hash/lock persistidos y rehash atómico/session invalidation conformes. |
| AS-10 | BLOCKED | Restore técnico no instalación completa | Digest completo antes/después y smoke tras restore. |

**Total: 1 PASSED, 28 BLOCKED, 1 NOT_RUN.** Aceptar ADR-0021 no modifica esta
matriz automáticamente; cada prueba necesita ejecución/evidencia propia.

## 6. Evidencias, procedencia y seguridad

Directorios nuevos, sin sobrescribir 0/0b:

- `backend-rust/target/increment0c-evidence-1/` y `increment0c-gate-1/`:
  ejecución inicial sobre HEAD `6a15ee6` con cambios locales.
- `backend-rust/target/increment0c-gate-2/`: código técnico commit **43d9909**,
  `baseline-manifest.worktree.dirty=true` por documentación en elaboración.
  No se presenta como ejecución del futuro commit documental limpio.
- `backend-rust/target/increment0c-remote-40641592100/`: artefacto remoto histórico
  descargado, verificado sin sobrescribirlo; no nuevo run remoto.

SHA-256 calculados sobre bytes de la salida **gate-2**:

| Archivo | SHA-256 |
| --- | --- |
| `parity-report.json` | `5e1be0f1d54f0fed11b01025cb69e817946a6cce8bbcc08d24aaac6de5003935` |
| `baseline-manifest.json` | `139f3cfb5e81dceb61ff0a0eb73e2a4d6b6493d589a5cfea01ae572c0c34477d` |
| `migration-manifest.json` | `b9d40be26c4ee907446f8f4d36ab1b25762626040c2d8e95cac7479ac1b56cc7` |
| `schema-snapshots/technical-digests.json` | `629407493211a888a40436718c5f981f1634652412d3dedbe0e17f4b74aac42e` |

El directorio contiene JSON/JUnit, manifests, schema snapshots, dumps/backups
sintéticos, security/auth, money/stock, failure logs y run-summary generados por
el runner existente. No se versionan binarios/bundles de credenciales. Los
artefactos normativos pendientes no reciben digest de datos target inexistentes.

AUTH-MIG 8/8 sigue siendo prueba de laboratorio. `lab-argon2-v1` no es una
política aprobada comercialmente ni benchmark en hardware objetivo. No se
importan sesiones/tokens Java, secretos JWT o usuarios reales. Sin dependencias
nuevas ni cambio de licencias del grafo; RustSec no sustituye revisión legal.
La comprobación de digests no protege contra un atacante que sustituya juntos
archivo y manifest, ni promete resistencia al administrador del archivo SQLite.

## 7. Revisión, documentación y decisiones requeridas

Subagentes reales con escrituras disjuntas: negocio/físico pendiente, paridad Java,
verificador de evidencia; revisión final read-only independiente. Skills cargados:
Git, arquitectura, documentación, testing, secure-coding, CI y code-review. No se
asumen perfiles Rust no instalados ni se leen configuraciones de credenciales.

La revisión encontró y se corrigieron documentalmente: replay Java descarta
líneas duplicadas posteriores, y orden reproducible no garantiza orden causal del
ledger. Segunda revisión de las correcciones: sin defectos nuevos bloqueantes.
La falta de enforcement remoto sigue siendo riesgo material, no corregido sin
permiso. No hay aprobación humana implícita por la revisión del subagente.

Documentación actualizada: ADR-0021, README Rust, foundation, physical audit,
parity baseline, borrador PR, CI e informe 0b con corrección explícita de su
estado remoto. Nuevos anexos: diccionario pendiente, contrato compras, CC-0021,
reconciliación Git y este informe. ADR-0020 conserva su decisión histórica.

**Requiere titular:** resolver D01..07/CC21-01..05, elegir A/B para dev experimental
y autorizar protecciones. No se aplica DDL pendiente, cambio canónico, nueva
política de auth o reversión de integración sin esa aceptación.

## 8. Plan exacto para cerrar los 29 casos pendientes

1. **Cierre normativo:** aceptar/ajustar ADR y CC con campos/scopes, orden causal,
   valoración pendiente, snapshots y reglas históricas/nuevas. Publicar baseline
   canónico versionado con hashes y cero FK sin definición. Si se rechaza una
   opción, mantener el import bloqueado, no implementar alternativas por defecto.
2. **Físico y clean (DDL13-01..03/11):** añadir migrations nuevas por contexto,
   diccionario derivado del baseline, pruebas constraints/scope/roles; nunca editar
   0001 publicada ni V1–V25. Semántica SQLite/PG demostrada, no copiada por sintaxis.
3. **Evolución/recuperación (DDL13-04..09, AS-10):** snapshots N-1 completos,
   clean vs upgrade, checksum/DDL deliberadamente corruptos, historial sin success
   falso, restore exacto, reopen/smoke y rerun sin cambios.
4. **Offline (DDL13-10):** compilar antes, denegar red durante ejecución, probar
   clean/upgrade embebidos en targets completos con evidencia del aislamiento.
5. **Import sintético versionado (PAR-01..04/06/07/09, AS-01..08):** acciones
   37/37, IDs/scopes/snapshots/cantidades/dinero exactos, configuración nueva
   identificada, rollback ante dato no representable, dos imports y conflictos,
   ledger/valor reconciliados sin duplicación ni costos inferidos. Mantener PAR-08.
6. **Auth (PAR-05/AS-09):** adapter persistente y transacción segura de rehash,
   lockouts/status/revocación, benchmark release hardware objetivo y policy
   aprobada; nunca tokens Java ni usuarios productivos.
7. **Cierre independiente:** los 30 casos con evidencia machine-readable válida,
   `--require-complete` exit **0 real**, revisión de seguridad/datos y run remoto
   del SHA final. Java/React/Compose y E2E correspondientes ejecutados; reglas
   de protección autorizadas activas. Ningún PR documental verde sustituye esto.

No comenzar Incremento 1 Organization/Branch/Users/Auth/RBAC productivo todavía.
**Siguiente incremento exacto: 0d — baseline físico aprobado + migrations aditivas
completas y clean/upgrade/restore normativos**, dependiente de la aprobación
explícita anterior. El import/paridad se verifica a continuación dentro del cierre
0; ni 0 ni 0c se declaran COMPLETADOS mientras existan estos bloqueos.
