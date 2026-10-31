# Propuestas de migración Local — Cajas y turnos

**Estado: PROPUESTA. No ejecutar. No está cargada por `pos-local` ni por SQLx.**

`0009_cash_register.proposed.sql` y `0010_cash_shift.proposed.sql` permanecen
fuera de `backend-rust/migrations/local/`. La promoción exige aceptación expresa
D-CASH y se hará en un cambio separado; `0001` no se modifica.

## Dependencias y orden candidato

| Orden | Migración candidata | Depende de | Estado |
| --- | --- | --- | --- |
| 0001 | `0001_organization_branch.sql` | — | Publicada, intocable |
| 0003 | `proposals/local-auth/0003_user_credential_session.proposed.sql` | `0001` | Propuesta; aporta `pos_user` |
| 0009 | `0009_cash_register.proposed.sql` | `0001` | Propuesta D-CASH-01 |
| 0010 | `0010_cash_shift.proposed.sql` | `0003`, `0009` | Propuesta D-CASH-02..04 |

La caja puede crear su estructura desde `0001`; el turno exige `pos_user` porque
los actores de apertura y cierre llevan FKs compuestas `(user_id, organization_id)`.
No se inventa una tabla local de operador ni se relaja esa propiedad.

## Paridad Java/PostgreSQL

| Aspecto | Flyway V11/V12 | Propuesta Local |
| --- | --- | --- |
| Tablas | `cash_registers`, `cash_shifts` | `cash_register`, `cash_shift` |
| Caja | Owner org/sucursal; código inmutable único por sucursal | Igual, con FK compuesta `branch_id, organization_id` |
| Turno | Owner org/sucursal/caja; actores same-org | Igual, con FKs compuestas a caja y usuarios |
| Un turno OPEN | Índice parcial único | Igual: `cash_shift_one_open_per_register` |
| Historial CLOSED | Ilimitado; sin hard delete ni reapertura | Igual |
| Importe | `NUMERIC(19,4) >= 0` | `TEXT` canónico no negativo escala 4 |
| Timestamp | `TIMESTAMPTZ` | RFC 3339 UTC en `TEXT` |

Los importes de apertura/cierre son declaraciones del operador. No son saldos,
reconciliaciones ni movimientos de efectivo; no se añadieron esos conceptos.

## Autoridad ante concurrencia

El precheck `cash_register_has_open_shift` mejora el error de negocio, pero no es
la barrera final. SQLite impone:

```sql
CREATE UNIQUE INDEX cash_shift_one_open_per_register
    ON cash_shift (cash_register_id)
    WHERE status = 'OPEN';
```

`SqliteCashApplication` usa `SqliteUnitOfWork` (`BEGIN IMMEDIATE`) para apertura,
cierre, guard de ventas, mutación y auditoría. `OpenSalesForCashShift` recibe la
misma `SqliteTransaction`; la persistencia de ventas debe crear una venta OPEN con
esa misma estrategia antes de confirmar. No existe un fallback que responda
siempre `false`.

## Validación candidata

`pos-storage-sqlite/src/cash_store_tests.rs` aplica explícitamente 0001, las
propuestas Auth/RBAC/Audit y estas dos propuestas sobre un archivo SQLite temporal.
No interviene `pos-local::migrate()`. Prueba: ownership físico, dinero canónico,
creación/lectura/listado/CAS de caja, apertura/cierre, índice parcial bajo dos
pools, historial CLOSED, rollback de auditoría, principal scoped, recuperación,
sesiones revocadas, reapertura y serialización carrera venta–cierre con un puerto
SQLite de prueba aislado.
