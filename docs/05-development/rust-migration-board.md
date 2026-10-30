# Tablero maestro de migración Java → Rust

**Actualizado en el Incremento 1X.** Este tablero registra el estado real de la
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
| Catálogo (categorías, productos) | `APPLICATION (candidate)` | `pos-core::catalog`, `proposals/local-catalog/0008`, `SqliteCatalogApplication`, `SqliteCatalogTransaction`, 9 pruebas de integración (CAS, tenant, rollback por auditoría, reapertura) | Aprobación D-CAT, promoción a `migrations/local`, API |
| Modificadores | `PERSISTENCE (fixture)` | `0011` candidato, FKs same-org y snapshots de opciones usados por ventas SQLite | CRUD/autorización administrativa, aprobación D-SALES, promoción, API |
| Cajas | `APPLICATION (candidate)` | `pos-core::cash`, `0009` candidato, `SqliteCashApplication`, SQLite real: scope, CAS, auditoría, reapertura | Aprobación D-CASH, promoción, API |
| Turnos | `APPLICATION (candidate)` | `pos-core::cash`, `0010` candidato, índice parcial OPEN, guard real `sale.status=OPEN`/UoW, dos pools y recuperación | Aprobación D-CASH/D-SALES, promoción, API |
| Inventario (artículos, saldos, ledger) | `APPLICATION (candidate)` | `pos-core::inventory`, `0013` candidato, `SqliteInventoryApplication`, ledger append-only por trigger, apertura única, CAS y dos pools | Aprobación D-INV, promoción, API |
| Ventas | `APPLICATION (candidate)` | `0011`/`0012` candidatos, `SqliteSalesApplication`, snapshots, CAS, auditoría, carreras venta/cierre y reapertura | Aprobación D-SALES, promoción, API |
| Checkout y pagos | `APPLICATION (candidate)` | `0015` candidato, `SqliteCheckoutApplication`, pagos + consumo + `COMPLETED` + auditoría en una UoW, política de stock y rollback | Aprobación D-SALES-04/05, promoción, API |
| Compras y recepciones | `DOMAIN` | `pos-core::purchasing`, ciclo de vida, snapshots, recepciones acumuladas y exceso | DDL, adaptador, API |
| Proveedores | `DOMAIN` | `pos-core::supplier`, slug inmutable, contacto acotado, email normalizado y snapshot | DDL, adaptador, API |
| Recetas y efectos de modificador | `PERSISTENCE (fixture)` | `pos-core::recipe`, `0014` candidato, resolución de consumo y snapshots usados por checkout | Mantenimiento administrativo, aprobación D-INV, promoción, API |
| API HTTP / OpenAPI | `NOT_STARTED` | — | Todo |
| Transporte Local | `IPC (lab)` | Tauri v2: tres handlers de lectura, bindings generados, ACL local y sesión revalidada en SQLite; [contrato](../03-api/rust-desktop-ipc.md), ADR-0023 | Activación comercial, composición canónica y seguridad aprobada |
| Integración React | `INTEGRATED (lab only)` | Pantalla separada: sucursales/categorías/productos; E2E Windows real React → WebView2 IPC → Application → SQLite | Login comercial, escrituras, integración de pantallas operativas y paridad completa |
| Arnés Docker Local | `DEV HARNESS (validated)` | `backend-rust/Dockerfile` multi-stage, runtime no-root, volumen, healthcheck funcional (`verify_health`: integridad + migraciones), reinicio y `down`/`up` con checksum idéntico | No es distribución Free Local ni expone red |
| Arnés sintético extremo a extremo | `E2E (synthetic)` | `tools/pos-lab`: bootstrap → admin → sesión → RBAC → sucursal → categoría → producto → inventario → receta → caja → turno → venta → checkout → consumo → `COMPLETED` → auditoría → cierre → reapertura. Ejecutado en local y en Docker | Sigue sobre esquema **candidato**; no es producto |

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

Ningún bloqueo autoriza a debilitar FKs, `--require-complete` ni los gates
DDL13/PAR/AS.

## 5. Regla de actualización

Un módulo solo avanza cuando el incremento correspondiente publica código,
pruebas y documentación. Los estados `DOMAIN` y `PERSISTENCE (lab|fixture)` no
habilitan operar en producción ni sustituyen una aprobación normativa.
