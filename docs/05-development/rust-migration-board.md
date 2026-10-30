# Tablero maestro de migración Java → Rust

**Actualizado en el Incremento 1J.** Este tablero registra el estado real de la
migración, no el deseado. Un estado solo avanza con evidencia publicada.

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

## 2. Estado por módulo

| Módulo Java | Estado Rust | Evidencia | Falta |
| --- | --- | --- | --- |
| Organization | `APPLICATION` | `pos-core::organization_branch`, `SqliteOrganizationApplication`, `SqliteUnitOfWork`, pruebas de CAS/rollback/lock | API, DDL canónico, aprobación 1I |
| Branch | `APPLICATION` | `BranchApplication`, `SqliteBranchApplication`, pruebas de ownership y timezone | API, DDL canónico |
| Audit | `PERSISTENCE (fixture)` | `pos-core::audit` (metadata ≤16 KiB, USER/SYSTEM), `audit_event` propuesto con triggers append-only | DDL aprobado, writer persistente, tabla canónica |
| Users / Auth | `PERSISTENCE (lab)` | `pos-core::identity`, `identity_lab`, `credential_lab`, `pos-credentials` | DDL aprobado, verificación productiva, sesiones operativas |
| RBAC | `PERSISTENCE (lab)` | Catálogo exacto de 59 permisos, override de organización, modo recuperación | DDL aprobado, resolución contextual productiva |
| Bootstrap | `PERSISTENCE (fixture)` | Pruebas de one-shot, ownership compuesto y rollback; tramo Organization en Rust | Secreto pre-body, DDL aprobado, endpoint/servicio de producto |
| Catálogo (categorías, productos) | `DOMAIN` | `pos-core::catalog`, 18 pruebas | DDL, adaptador, casos de uso, API |
| Modificadores | `DOMAIN` | `pos-core::catalog`, semánticos derivados y plan de reemplazo | DDL, adaptador, casos de uso, API |
| Cajas | `DOMAIN` | `pos-core::cash`, scope de sucursal y patch CAS | DDL, adaptador, casos de uso, API |
| Turnos | `DOMAIN` | `pos-core::cash`, apertura/cierre, recuperación y no idempotencia | DDL, índice único parcial, port de ventas abiertas, API |
| Inventario (artículos, saldos, ledger) | `DOMAIN` | `pos-core::inventory`, cantidades exactas, unidad bloqueable, CAS y ledger tipado | DDL, trigger append-only, adaptador, API |
| Ventas | `DOMAIN` | `pos-core::sales`, ciclo de vida, snapshots, totales exactos y modificadores | DDL, consumo de inventario, adaptador, API |
| Checkout y pagos | `DOMAIN` | `pos-core::sales`, cobertura exacta, efectivo/tarjeta y cambio | DDL, composición transaccional con inventario y turno, API |
| Compras y recepciones | `DOMAIN` | `pos-core::purchasing`, ciclo de vida, snapshots, recepciones acumuladas y exceso | DDL, adaptador, API |
| Proveedores | `DOMAIN` | `pos-core::supplier`, slug inmutable, contacto acotado, email normalizado y snapshot | DDL, adaptador, API |
| Recetas y efectos de modificador | `DOMAIN` | `pos-core::recipe`, reemplazo, resolución de consumo y snapshots de línea | DDL, adaptador, composición de checkout |
| API HTTP / OpenAPI | `NOT_STARTED` | — | Todo |
| Integración React | `NOT_STARTED` | — | Todo |

## 3. Dinero y cantidades

| Capacidad | Estado |
| --- | --- |
| `Currency` (sintaxis, no ISO 4217) | Implementado |
| `MoneyMinor` (`i64`, suma checked, moneda explícita) | Implementado |
| Importación decimal exacta con escala explícita | Implementado |
| `Decimal4` de catálogo (escala fija 4, rango `NUMERIC(19,4)`) | Implementado (1J) |
| Cantidades y conversión de unidades | Pendiente |
| Redondeo e impuestos | Pendiente |
| Política de stock negativo | Representada en el dominio; sin ledger |

## 4. Bloqueos normativos activos

| Bloqueo | Alcance | Efecto |
| --- | --- | --- |
| D-AUTH-01..08, D-AUTH-10, D-AUTH-11 | Identidad, sesión, auditoría, bootstrap, email | Impide promover `0002`–`0007` |
| D-AUTH-09 | Argon2id comercial | Impide fijar parámetros productivos |
| D-INST-01, D-INST-02, ADR-0022 | Instalación y reconciliación física | Impide cerrar `installation_identity` |
| Representación física del catálogo | Importes, unicidad, búsqueda | Impide el adaptador de catálogo |

Ningún bloqueo autoriza a debilitar FKs, `--require-complete` ni los gates
DDL13/PAR/AS.

## 5. Regla de actualización

Un módulo solo avanza cuando el incremento correspondiente publica código,
pruebas y documentación. Los estados `DOMAIN` y `PERSISTENCE (lab|fixture)` no
habilitan operar en producción ni sustituyen una aprobación normativa.
