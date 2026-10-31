# Incremento 1S — Ventas candidatas SQLite y coordinación real con turnos

**Estado: implementación candidata verificada.** No se promovieron migraciones,
no se habilitó checkout ni existe una frontera HTTP/IPC productiva.

## Alcance

La rama parte de `feat/backend/rust-cash-persistence-1r` en
`b00e7e5a462df96e8a3cd1de6ed73b415e02069a`. El incremento reemplaza el guard
de prueba de cajas por una consulta real sobre la tabla candidata `sale` y
prepara el ciclo comercial anterior al checkout:

```text
Caja → Turno OPEN → Venta OPEN → Partida cotizada y snapshots
→ total exacto/CAS → guard de cierre → CANCELLED → cierre de turno
```

`COMPLETED` está representado en el DDL para que pagos y checkout puedan ser
aditivos, pero `SqliteSalesApplication` no expone ninguna operación para llegar
a ese estado. Completar una venta sin pagos e inventario atómicos violaría
ADR-0008 y ADR-0014.

## DDL candidato

| Archivo | Contenido | Dependencias |
| --- | --- | --- |
| `proposals/local-sales/0011_modifier_catalog.proposed.sql` | `modifier_group`, `modifier_option`, `product_modifier_group`; FKs same-org, límites de selección, precio delta `TEXT` canónico | `0008` candidato |
| `proposals/local-sales/0012_sales_foundation.proposed.sql` | `sale`, `sale_item`, `sale_item_modifier`; scope compuesto caja/turno, snapshots, CAS y estados finales | `0003`, `0008`–`0011` candidatos |

Los archivos no pertenecen a `migrations/local/`; `pos-local::migrate()` no los
descubre. Las propuestas usan `TEXT` decimal canónico de escala cuatro, nunca
`REAL`, `f32` ni `f64`.

## Aplicación y paridad

`pos-storage-sqlite/src/sales_store.rs` añade `SqliteSalesApplication`:

- Crear, consultar y listar ventas scoped.
- Añadir, recotizar y retirar lógicamente partidas con CAS sobre `Sale.version`.
- Resolver producto, grupos y opciones en servidor; el request solo lleva IDs y
  cantidad. SKU, nombre, precio base, delta, precio efectivo y snapshots se
  persisten desde el catálogo autorizado.
- Cancelar ventas OPEN conservando líneas y snapshots históricos.
- Escribir `sale.created`, `sale.item.added`, `sale.item.updated`,
  `sale.item.removed` y `sale.cancelled` con `SqliteAuditWriter` en la misma
  UoW.
- Exigir `VerifiedPrincipal`, scopes organization/branch y permisos canónicos
  `sale.read`, `sale.create`, `sale.update` y `sale.cancel`.

`SqliteOpenSalesForCashShift` implementa el puerto de `cash_store` mediante
`EXISTS` sobre `sale.status = 'OPEN'` usando la misma `SqliteTransaction` que
cierra el turno. No abre un pool alternativo ni tiene respuesta por defecto.

La coordinación Java `PESSIMISTIC_WRITE` de ADR-0007 se adapta a una única UoW
SQLite con `BEGIN IMMEDIATE`: crear venta y cerrar turno se serializan. La venta
mantiene CAS por revisión para mutaciones ordinarias, como en Java.

## Checkout explícitamente pendiente

No hay pagos, snapshots de consumo, balances ni ledger persistentes Local.
Por tanto, `sale.checkout` y `Sale::complete` no se conectan a la aplicación.
El próximo incremento debe reutilizar `pos-core::sales::Payment`, recetas y
snapshots de inventario para implementar una sola transacción:

```text
validar venta OPEN y turno → validar pagos → resolver snapshots de inventario
→ aplicar ledger/política de stock → COMPLETED → auditoría → COMMIT
```

No se admite una ruta alternativa que marque `COMPLETED` sin esas operaciones.

## Pruebas SQLite candidatas

La suite añade cinco pruebas de ventas y actualiza el fixture de caja para usar
ventas reales:

1. Caja → turno → venta → modificador → total exacto → guard → cancelación →
   cierre, auditoría y reapertura.
2. CAS perdido, actualización/retirada lógica de partida y rollback si falla
   auditoría.
3. Dos pools compiten entre crear venta OPEN y cerrar turno; no queda una venta
   OPEN bajo un turno CLOSED.
4. Cancelación de una UoW después de insertar una venta: RAII revierte antes de
   reutilizar la conexión.
5. FKs de scope y el formato monetario canónico rechazan inserciones SQL
   inválidas.

Además, `pos-core` comprueba que `SaleItem::restore` rechaza una fila cuyo total
no coincide exactamente con el cálculo comercial.

## Validación ejecutada (Windows)

| Control | Resultado real |
| --- | --- |
| `cargo fmt --all -- --check` | ✅ |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | ✅ |
| `cargo test --locked --workspace --all-features` | ✅ 260 pruebas, 0 fallos |
| `cargo build --locked --workspace --all-features` | ✅ |
| Prueba focalizada `sales_store_tests` | ✅ 5 pruebas SQLite reales |
| Linux/CI y Migration Lab PostgreSQL 17 | No ejecutados en este incremento Windows; los PRs apilados feature no disparan el workflow actual |

Distribución observada: `migration-lab` 27 · `pos-core` 125 ·
`pos-credentials` 8 · `pos-local` 11 · `pos-storage-sqlite` 89.

## Decisiones pendientes

| ID | Decisión requerida | Recomendación técnica, no aprobación |
| --- | --- | --- |
| D-SALES-01 | Tablas Local de modificadores y FKs same-org | Aprobar como restauración de Flyway V10 necesaria para cotizar snapshots |
| D-SALES-02 | `sale`/`sale_item`/snapshots y representación `TEXT(19,4)` | Aprobar como adaptación Local exacta de V13/V14 sin `REAL` |
| D-SALES-03 | `BEGIN IMMEDIATE` compartido para crear venta/cerrar turno | Aprobar como sustitución SQLite verificable del lock de ADR-0007 |
| D-SALES-04 | Checkout diferido hasta pagos e inventario atómicos | Mantener: no autorizar `COMPLETED` aislado |

Siguen pendientes D-AUTH, D-INST, D-CAT y D-CASH. D-AUTH-09 permanece bloqueada
para parámetros comerciales Argon2id. Este incremento no marca ninguna decisión
como aprobada.
