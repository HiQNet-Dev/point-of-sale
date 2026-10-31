# Tablero maestro de migración Java → Rust

**Actualizado con el avance de consolidación 2C.** Este tablero registra el estado real de la
migración, no el deseado. Un estado solo avanza con evidencia publicada.

Arquitectura: monolito modular hexagonal (`pos-core` ← `pos-application` ←
`pos-storage-sqlite` ← `pos-local`), documentada en
`docs/01-architecture/rust-hexagonal-architecture.md` y obligatoria en
`AGENTS.md`.

## 1. Escala de estados

| Estado | Criterio mínimo |
| --- | --- |
| `NOT_STARTED` | Sin código Rust propio |
| `DOMAIN` | Agregados, value objects y reglas puras con pruebas |
| `PERSISTENCE` | Adaptador sobre SQLite **aprobado** o fixture experimental explícitamente marcada |
| `APPLICATION` | Casos de uso con autorización, UoW y auditoría transaccional |
| `API` | Contrato HTTP documentado en OpenAPI y probado |
| `INTEGRATED` | Consumido por React y verificado de extremo a extremo |
| `PARITY_VERIFIED` | Paridad diferencial Java/Rust registrada, incluyendo rechazos |

`PERSISTENCE (lab)` y `PERSISTENCE (fixture)` **no** equivalen a persistencia
productiva: las tablas implicadas siguen sin aprobación física.

`APPLICATION (candidate)` indica casos de uso completos —autorización, UoW, CAS y
auditoría transaccional— sobre un esquema **candidato** que aún no vive en
`migrations/local`. Tampoco equivale a producto.

## 2. Estado por módulo

| Módulo Java | Estado Rust | Evidencia | Falta |
| --- | --- | --- | --- |
| Organization | `APPLICATION` | `pos-core::organization_branch`, `SqliteOrganizationApplication`, `SqliteUnitOfWork`, pruebas de CAS/rollback/lock | API, DDL canónico, aprobación 1I |
| Branch | `APPLICATION` | `BranchApplication`, `SqliteBranchApplication`, pruebas de ownership y timezone | API, DDL canónico |
| Audit | `PERSISTENCE (fixture)` | `pos-core::audit` (metadata ≤16 KiB, USER/SYSTEM), `audit_event` propuesto con triggers append-only | DDL aprobado, writer persistente, tabla canónica |
| Users / Auth | `PERSISTENCE (lab)` | `pos-core::identity`, `identity_lab`, `credential_lab`, `pos-credentials` | DDL aprobado, verificación productiva, sesiones operativas |
| RBAC | `PERSISTENCE (lab)` | Catálogo exacto de 59 permisos, override de organización, modo recuperación | DDL aprobado, resolución contextual productiva |
| Bootstrap | `PERSISTENCE (fixture)` | Pruebas de one-shot, ownership compuesto y rollback; tramo Organization en Rust | Secreto pre-body, DDL aprobado, endpoint/servicio de producto |
| Catálogo (categorías, productos) | `APPLICATION (candidate)` | `pos-core::catalog`, `proposals/local-catalog/0008`, `SqliteCatalogApplication`, 9 pruebas de integración (CAS, tenant, rollback por auditoría, reapertura); escritura expuesta solo en laboratorio por IPC con auditoría transaccional | Aprobación D-CAT, promoción a `migrations/local`, API |
| Modificadores | `PERSISTENCE (fixture)` | `0011` candidato, FKs same-org y snapshots de opciones usados por ventas SQLite | CRUD/autorización administrativa, aprobación D-SALES, promoción, API |
| Cajas | `APPLICATION (candidate)` | `pos-core::cash`, `0009` candidato, `SqliteCashApplication`, SQLite real: scope, CAS, auditoría, reapertura | Aprobación D-CASH, promoción, API |
| Turnos | `APPLICATION (candidate)` | `pos-core::cash`, `0010` candidato, índice parcial OPEN, guard real `sale.status=OPEN`/UoW, dos pools y recuperación | Aprobación D-CASH/D-SALES, promoción, API |
| Inventario (artículos, saldos, ledger) | `APPLICATION (candidate)` | `pos-core::inventory`, `0013` candidato, `SqliteInventoryApplication`, ledger append-only por trigger, apertura única, CAS y dos pools | Aprobación D-INV, promoción, API |
| Ventas | `APPLICATION (candidate)` | `0011`/`0012` candidatos, `SqliteSalesApplication`, snapshots, CAS, auditoría, carreras venta/cierre y reapertura | Aprobación D-SALES, promoción, API |
| Checkout y pagos | `APPLICATION (candidate)` | `0015` candidato, `SqliteCheckoutApplication`, pagos + consumo + `COMPLETED` + auditoría en una UoW, política de stock y rollback | Aprobación D-SALES-04/05, promoción, API |
| Compras y recepciones | `APPLICATION (candidate)` | `pos-core::purchasing` (estados, emisión, cancelación, recepciones acumuladas, costo opcional con prueba «desconocido ≠ cero»), `PurchasingApplication` con 13 casos de uso de orden/recepción, `proposals/local-purchasing/0016`, 12 pruebas SQLite reales (emisión irreversible, CAS, sobre-recepción, idempotencia, rollback por auditoría) | Aprobación física de `0016`, promoción, API |
| Proveedores | `APPLICATION (candidate)` | `pos-core::supplier`, `PurchasingApplication` (crear/consultar/listar/actualizar con CAS), `0016` candidato, pruebas de slug duplicado y aislamiento | Aprobación física, promoción, API |
| Recetas y efectos de modificador | `PERSISTENCE (fixture)` | `pos-core::recipe`, `0014` candidato, resolución de consumo y snapshots usados por checkout | Mantenimiento administrativo, aprobación D-INV, promoción, API |
| API HTTP / OpenAPI | `NOT_STARTED` | — | Todo |
| Transporte Local | `IPC (lab)` | Tauri v2: tres handlers de lectura, bindings generados, ACL local y sesión revalidada en SQLite; [contrato](../03-api/rust-desktop-ipc.md), ADR-0023 | Activación comercial, composición canónica y seguridad aprobada |
| Instalación Local persistente | `PERSISTENCE (approved 0001)` | `apps/pos-desktop --installation <dir>`: migrador aprobado, clasificación primera instalación/reapertura, rechazo de esquema posterior, cierre con checkpoint; E2E Windows de dos arranques y smoke Docker | Directorio canónico por plataforma (`app_data_dir`), identificador estable de aplicación y operaciones de negocio; el bloque D está aceptado como diseño desde 2026-10-30 |
| Integración React | `INTEGRATED (lab only)` | Pantalla separada de laboratorio (lectura + creación de producto) y pantalla de instalación; E2E Windows real React → WebView2 IPC → Application → SQLite → auditoría | Login comercial, escrituras en producto, integración de pantallas operativas y paridad completa |
| Arnés Docker Local | `DEV HARNESS (validated)` | `backend-rust/Dockerfile` multi-stage, runtime no-root, volumen, healthcheck funcional (`verify_health`: integridad + migraciones), reinicio, cierre con checkpoint, evidencia `first_install` → `reopened` y checksum idéntico | No es distribución Free Local ni expone red |
| Arnés sintético extremo a extremo | `E2E (synthetic)` | `tools/pos-lab`: bootstrap → admin → sesión → RBAC → sucursal → categoría → producto → inventario → receta → caja → turno → venta → checkout → consumo → `COMPLETED` → auditoría → cierre → reapertura, más matriz ejecutable de dependencias `0002`–`0015` (FK y `foreign_key_check` por paso). Ejecutado en local y en Docker | Sigue sobre esquema **candidato**; no es producto |

### Consolidación 2C: camino persistente, no activación comercial

La instalación aprobada añade validación SQLx de checksums en cada reapertura,
upgrades limitados al migrador aprobado, rechazo de rutas redirigidas/auxiliares
inseguros y exclusión de segunda instancia desktop mediante candado del SO.
Pruebas: 315 headless, 29 desktop Windows, 9 de directorios en Linux Docker;
E2E WebView2 de instalación y laboratorio, y smoke Docker aprobados localmente.
Detalle y límites en [el informe 2C](rust-increment-2c.md).

No se promovió ninguna migración, no hay login comercial ni nuevos comandos de
Purchasing. Las 17 operaciones permanecen `APPLICATION (candidate)`, sin paridad
diferencial completa ni integración IPC/React. A/B/C y D-AUTH-09 siguen pendientes.

## 3. Dinero y cantidades

| Capacidad | Estado |
| --- | --- |
| `Currency` (sintaxis, no ISO 4217) | Implementado |
| `MoneyMinor` (`i64`, suma checked, moneda explícita) | Implementado |
| Importación decimal exacta con escala explícita | Implementado |
| `Decimal4` de catálogo (escala fija 4, rango `NUMERIC(19,4)`) | Implementado (1J) |
| Puente `MoneyMinor` ↔ `Decimal4` con exponente explícito y error ante pérdida | Implementado (1Q, 9 pruebas) |
| Cantidades y conversión de unidades | Cantidades exactas escala 6 implementadas; conversión entre unidades pendiente |
| Redondeo e impuestos | Pendiente |
| Política de stock negativo | Aplicada en checkout candidato (`STRICT`/`ALLOW_NEGATIVE`) sobre saldos reales |

## 4. Bloqueos normativos activos

| Bloqueo | Alcance | Efecto |
| --- | --- | --- |
| D-AUTH-01..08, D-AUTH-10, D-AUTH-11 | Identidad, sesión, auditoría, bootstrap, email | Impide promover `0002`–`0007` |
| D-AUTH-09 | Argon2id comercial | Impide fijar parámetros productivos |
| D-INST-01, D-INST-02, ADR-0022 | Instalación y reconciliación física | Impide cerrar `installation_identity` |
| Representación física del catálogo | Importes, nombres de tabla | Impide promover `0008` (D-CAT-01, D-CAT-02) |
| Cajas y turnos candidatos | Nombres, importes, índice parcial y coordinación venta/cierre | Impide promover `0009`–`0010` (D-CASH-01..04) |
| Ventas y modificadores candidatos | Tablas, snapshots, dinero `TEXT` exacto y coordinación SQLite | Impide promover `0011`–`0012` (D-SALES-01..03) |
| Inventario, recetas y checkout candidatos | Cantidades `TEXT` escala 6, triggers, pagos y snapshots de consumo | Impide promover `0013`–`0015` (D-INV-01..02, D-SALES-04/05) |
| Compras candidatas | `0016` sobre `unit_cost` opcional y recepciones sin valoración | `CC21-03` aceptada el 2026-10-30 desbloquea la representación del costo pactado; sigue pendiente la aprobación física de `ADR-0021` (fichas, costo efectivo, snapshots) para promover `0016` |

### Decisiones aceptadas (2026-10-30)

| Decisión | Alcance aceptado | Efecto en el tablero |
| --- | --- | --- |
| Bloque D | Tauri v2/IPC, una SQLite por instalación, directorio privado del SO, rutas explícitas solo por argumentos, sin HTTP local, separación laboratorio/producto | Habilita implementar `app_data_dir`; no autoriza distribución ni release |
| `CC21-03` | `unit_cost TEXT NULL` con semántica desconocido ≠ cero y total derivado `NULL` | Habilita la propuesta `0016`; `ADR-0021` sigue `PROPOSED` |

Ningún bloqueo autoriza a debilitar FKs, `--require-complete` ni los gates
DDL13/PAR/AS.

## 5. Regla de actualización

Un módulo solo avanza cuando el incremento correspondiente publica código,
pruebas y documentación. Los estados `DOMAIN` y `PERSISTENCE (lab|fixture)` no
habilitan operar en producción ni sustituyen una aprobación normativa.
