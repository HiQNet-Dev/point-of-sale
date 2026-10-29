# Incremento 1D — transacciones SQLite seguras y Organization async

**Estado: PARCIAL EXPERIMENTAL.** Este incremento extiende la composición SQLite
de `Organization` y `Branch`; no habilita producción, HTTP, RBAC
DB-authoritative ni auditoría Local persistente.

## Transacción SQLx y bloqueo

`SqliteUnitOfWork::begin` adquiere una `PoolConnection` y construye un
`sqlx::Transaction` nativo con la sentencia estática `BEGIN IMMEDIATE` mediante
`Transaction::begin(..., Some(SqlStr::from_static(...)))` de SQLx 0.9.0.

- No se emite un segundo `BEGIN` dentro de una transacción ya abierta.
- No hay booleano manual de finalización, `block_on`, mutex global ni clonación de
  conexión.
- La mutación, CAS y `TransactionalAuditWriter` reciben la misma
  `SqliteTransaction`.
- Al abandonar una transacción sin `commit`/`rollback`, SQLx encola el rollback
  en el worker. La siguiente operación sobre esa conexión procesa la limpieza
  antes de reutilizarla. SQLx cierra el worker si no consigue limpiar un `BEGIN`
  cancelado; el pool no debe reutilizar una conexión en ese estado.
- Un `commit` que devuelve error se informa como error de almacenamiento. La
  aplicación no lo convierte en éxito ni reintenta una mutación/auditoría no
  idempotente. Un fallo de confirmación por pérdida de acuse sigue siendo
  indeterminado a nivel de producto: requerirá reconciliación por identidad e
  idempotencia cuando exista el contrato de comandos.

El pool experimental continúa con una conexión por proceso, WAL,
`foreign_keys=ON`, `synchronous=FULL` y `busy_timeout` acotado. Esa serialización
local no acredita exclusión entre procesos. Por eso las pruebas de bloqueo y CAS
abren **pools independientes** sobre el mismo archivo.

## Evidencia SQLite real

`pos-storage-sqlite` incorpora pruebas con SQLite STRICT para:

- `BEGIN IMMEDIATE`: el segundo escritor independiente recibe busy/timeout;
  una lectura WAL compatible continúa funcionando.
- Drop sin `commit`, rollback explícito y cancelación de una `task` después de
  insertar y antes de confirmar; se vuelve a adquirir la conexión y el dato no
  confirmado no es visible.
- Fallo de auditoría fixture, `UNIQUE`, FK, CAS, scope de Branch y reapertura
  durable.
- Fallo reproducible de `COMMIT` mediante FK diferida: no se declara éxito y la
  fila pendiente no queda persistida.
- Dos CAS concurrentes de Organization y Branch con dos pools: exactamente uno
  actualiza la misma revisión y el otro recibe conflicto.

Las tablas `audit_fixture` se crean dentro de los tests; no son una migración ni
un historial de auditoría del producto.

## Organization async

`SqliteOrganizationApplication` aporta:

- `bootstrap_create`: exige `Permission::BootstrapOrganization`, inserta la
  organización y registra `organization.created` en una única transacción.
  Modela únicamente el tramo Organization del bootstrap Java; no sustituye el
  secreto previo al body, el estado one-shot ni la provisión de usuario/rol.
- `get`: exige `OrganizationRead` y usa una lectura sin reserva de escritor.
- `update`: exige `OrganizationUpdate`, reutiliza `Organization::patch`, aplica
  CAS en SQL y emite `organization.updated` solo cuando hay cambio material.

El bootstrap Java es el único flujo que crea `organization.created`;
`OrganizationService.create` no lo emite. La composición Rust conserva esa
separación. `pos-core` centraliza los nombres de campos auditables de Organization
y Branch para que las composiciones síncrona y async no diverjan.

## Autorización y auditoría

Los fixtures de autorización son explícitos y deny-by-default; no existe un
adaptador allow-all de producción. El puerto `Authorization` no equivale a RBAC:
la futura composición debe enlazar un principal autenticado con memberships,
permisos y estados actuales de Organization/Branch en una frontera
DB-authoritative.

El writer de auditoría continúa siendo experimental. La semántica comprobada es
`autorización -> validación -> CAS -> auditoría fixture -> commit`; cualquier
error previo al commit revierte la mutación. Faltan el DDL Local aceptado,
protección append-only, actor USER/SYSTEM, correlación de sesión Local y
sanitización/validación de metadata para declarar auditoría productiva.

## Plataforma y gates

Las pruebas de este incremento se ejecutan en Windows contra SQLite. El Migration
Lab PostgreSQL 17 permanece nativo únicamente en Linux/WSL2; el módulo Windows
rechaza explícitamente `--postgres-bin` y no lo marca como aprobado. Ejecutar el
lab y `--require-complete` en Linux/WSL2 según `backend-rust/README.md`.

No cambia `migrations/local/0001_organization_branch.sql`, Flyway V1–V25 ni los
gates. DDL13, PAR y AS siguen bloqueados/no ejecutados salvo PAR-08 histórico;
ADR-0021 y CC-0021 siguen `PROPOSED`.
