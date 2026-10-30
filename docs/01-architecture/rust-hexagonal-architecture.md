# Arquitectura Rust — monolito modular hexagonal

**Estado:** vigente desde el Incremento 1U. Complementa el Design Freeze y no lo
modifica: las decisiones físicas siguen pendientes en el expediente 1I/1M.

## 1. Decisión

El backend Rust es un **monolito modular** con arquitectura hexagonal
(Ports & Adapters). No se adoptan microservicios, CQRS ni event sourcing.

```text
Entrypoints (pos-local / Tauri-IPC cuando se apruebe)
        |
        v
Application (casos de uso, autorización, puertos)
        |
        v
Domain (entidades, value objects, invariantes)
        ^
        |
Infrastructure (SQLite, credenciales, reloj, auditoría)
```

## 2. Mapa real de crates

```text
pos-core             dominio puro; sin SQLx, sin tokio, sin transporte
pos-application      coordinación de casos de uso y puertos
pos-credentials      hashing y verificación (BCrypt/Argon2id)
pos-storage-sqlite   adaptadores SQLite; implementa los puertos
pos-local            composition root: config, SQLite, ciclo de vida
migration-lab        herramienta de paridad/migración
```

Dirección de dependencias permitida:

```text
pos-core <- pos-application <- pos-storage-sqlite <- pos-local
pos-core <- pos-local   (tipos compartidos)
```

Prohibido: `pos-core -> pos-application`, `pos-core -> sqlx`,
`pos-application -> sqlx`, y cualquier ciclo.

Estas reglas se verifican automáticamente en
`crates/pos-application/src/architecture_tests.rs`.

## 3. Responsabilidades por capa

| Capa | Contiene | No contiene |
| --- | --- | --- |
| Domain | agregados, VOs, invariantes, políticas puras, errores de dominio | SQL, pools, HTTP, configuración, auditoría de infraestructura |
| Application | casos de uso, autorización contextual, puertos, construcción de eventos auditables | nombres de tabla, SQLx, DTOs de transporte |
| Infrastructure | conexiones, PRAGMAs, UoW `BEGIN IMMEDIATE`, repositorios, mappers, SQL, triggers | reglas de negocio propias |
| Entrypoints | configuración, composición, migraciones autorizadas, ciclo de vida, observabilidad | reglas de catálogo, ventas, inventario o autenticación |

## 4. Puertos actuales

| Puerto | Consumidor | Implementación |
| --- | --- | --- |
| `authorization::{require_organization_scope, require_branch_scope, require_sale_scope, require_operational_branch}` | casos de uso SQLite | funciones puras sobre `VerifiedPrincipal` |
| `audit::AuditAppender` | caja, ventas, inventario, checkout | `SqliteTransaction` sobre la misma conexión |
| `sales_context::SaleOperationalContext` | ventas y checkout | `SqliteTransaction` |
| `cash_store::OpenSalesForCashShift` | cierre de turno | `SqliteOpenSalesForCashShift` |

Los puertos se declaran en la capa que los necesita. No se crean traits sin un
consumidor real.

## 5. Política transaccional (invariante)

Toda mutación auditable usa **una única** `SqliteUnitOfWork`:

1. `BEGIN IMMEDIATE` reserva el escritor.
2. Autorización y validaciones de dominio dentro de la transacción.
3. Mutación de negocio, snapshots y movimientos de ledger.
4. Inserción de auditoría en la misma conexión.
5. Un solo `commit`; cualquier fallo revierte todo.
6. `cancelación del future` produce rollback RAII.

Prohibido: abrir un pool o una transacción adicional dentro de una UoW,
confirmar mutaciones antes de auditar, o sustituir la transacción compartida por
varios commits.

Se conservan: CAS por revisión, aislamiento organización/sucursal, índice parcial
de turno abierto, serialización venta↔cierre y ledger append-only.

## 6. Flujo de referencia (checkout)

```text
pos-local (entrypoint)
  → SqliteCheckoutApplication (application)
      → require_sale_scope (puerto de autorización)
      → SaleOperationalContext (puerto de contexto, misma UoW)
      → Sale / validate_checkout / recipe (dominio)
      → Payment + ledger + audit (application + adaptador)
      → commit único
```

## 7. Testing

- Dominio: pruebas puras por feature en `pos-core`.
- Aplicación: pruebas de decisión (autorización, metadata, políticas).
- Infraestructura: SQLite real con esquema candidato explícito, sin mocks que
  sustituyan comportamiento transaccional.
- Arquitectura: guard tests de dependencias.
- Concurrencia: dos pools, CAS, rollback y cancelación.

No se aceptan mocks que reemplacen transacciones reales ni datos de clientes.

## 8. Convenciones

- Archivos y módulos por contexto de negocio, luego por responsabilidad.
- Rustdoc y comentarios en español; explican decisiones e invariantes.
- Sin `utils.rs` misceláneos ni `mod.rs` que acumulen implementación.
- Guía de tamaño: >600 líneas exige justificación; >1.000 prioriza extracción.
- Nombres de casos de uso en imperativo (`create_sale`, `open_stock`).

## 9. Evolución

Extracción incremental pendiente, en este orden:

1. Casos de uso de catálogo → `pos-application` conservando contratos.
2. Casos de uso de caja, ventas e inventario con los mismos puertos.
3. División física de `pos-core` — **completada**:
   - `sales` (`mod`, `errors`, `state`, `sale`, `sale_item`, `payment`,
     `checkout`, `pricing`).
   - `catalog` (`mod`, `errors`, `code`, `money`, `patch`, `category`,
     `product`, `modifier`, `ordering`).
   - `inventory` (`mod`, `errors`, `quantity`, `item`, `balance`, `movement`,
     `rules`).
   - `organization_branch` (`mod`, `errors`, `ids`, `text`, `values`,
     `organization`, `branch`, `audit_event`, `permissions`, `ports`,
     `application`).
   - `identity` (`mod`, `errors`, `user`, `credential`, `session`, `principal`,
     `service`).
   Todos conservan su ruta pública mediante reexportaciones.
4. División de `pos-storage-sqlite` — **completada en capas**:
   - Directorios por contexto (`audit/`, `bootstrap/`, `cash/`, `catalog/`,
     `checkout/`, `credential/`, `identity/`, `inventory/`, `sales/`).
   - `connection.rs` separado del facade.
   - Solo SQL, mappers y `impl` de puertos; sin reglas de negocio.
5. Casos de uso en `pos-application` — **completado**:
   - `cash`, `sales`, `checkout`, `catalog`, `inventory` y `organization`
     (Organization/Branch) con puertos, DTOs y `use_cases`.
   - `pos-core` conserva únicamente dominio y puertos compartidos.
6. División de mantenibilidad — **completada**:
   - Cada contexto de `pos-storage-sqlite` separa `queries.rs` (SQL estático),
     `mapper.rs` (filas `sqlx::FromRow` y rehidratación) y `repository.rs`
     (`impl` de puertos sobre `SqliteTransaction`); `store.rs` conserva solo el
     tipo de error del adaptador y el cableado de la UoW.
   - `lib.rs` queda como facade de módulos y reexportaciones; la UoW vive en
     `transaction.rs` y el adaptador Organization/Branch en
     `organization_branch.rs`.
   - `prelude.rs` centraliza imports y el traductor `transaction_error`, sin
     duplicarlo en cada contexto.
7. Pendiente normativo (no de capas): unificar la composición SQLite de
   Organization/Branch con los casos de uso de `pos-application`. Requiere
   decidir el actor SYSTEM del bootstrap (D-AUTH/ADR-0022); hoy conviven el
   camino canónico basado en `VerifiedPrincipal` y la composición experimental
   previa sobre `AuditEvent`.

Cada paso conserva pruebas y no promueve DDL.
