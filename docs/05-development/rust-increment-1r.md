# Incremento 1R — Persistencia candidata SQLite de cajas y turnos

**Estado: implementación candidata verificada. No hay DDL productivo ni decisión
normativa aprobada por este incremento.**

## 1. Alcance entregado

Rama de partida: `feat/backend/rust-catalog-persistence-1q` en
`fcdb11aa29adc1a00929848cdf63b2c25b28dedd`.

El incremento añade una operación completa sobre SQLite temporal y esquema
candidato:

```text
Crear caja → abrir turno → consultar OPEN → cerrar con CAS
→ auditar en la misma UoW → reabrir archivo y conservar historial
```

No cambia `main`, `dev`, Flyway V1–V25 ni
`backend-rust/migrations/local/0001_organization_branch.sql`.

## 2. DDL candidato

| Archivo | Contenido | Dependencias |
| --- | --- | --- |
| `proposals/local-cash/0009_cash_register.proposed.sql` | Caja branch-owned, código inmutable, CAS, FK compuesta a `branch` | `0001` |
| `proposals/local-cash/0010_cash_shift.proposed.sql` | Turno OPEN/CLOSED, actores same-org, importes exactos, historial y CAS | `0003`, `0009` |

Ambos permanecen deliberadamente fuera de `migrations/local` y SQLx no los carga
durante `pos-local::migrate()`.

### Propiedades físicas

- `cash_register` exige que `(branch_id, organization_id)` exista en `branch`.
- `cash_shift` exige que caja, organización y sucursal coincidan mediante FK
  compuesta; los dos actores se referencian a `pos_user` dentro de la misma
  organización.
- `cash_shift_one_open_per_register` es un índice parcial único: CLOSED no tiene
  límite y un segundo OPEN no puede confirmar.
- Los importes se persisten como `TEXT` decimal canónico no negativo de escala 4,
  no como `REAL` ni `INTEGER`; conserva el rango lógico `NUMERIC(19,4)`.
- El estado de cierre es físicamente coherente: OPEN no lleva datos de cierre y
  CLOSED exige actor, importe y timestamp.

## 3. Aplicación y seguridad

`pos-storage-sqlite/src/cash_store.rs` introduce:

- `SqliteCashApplication<S>` para crear, leer, listar y actualizar cajas, abrir,
  consultar/listar y cerrar turnos.
- `OpenSalesForCashShift`, puerto obligatorio que recibe la misma
  `SqliteTransaction` del cierre. No existe implementación allow-all ni fallback
  `false`.
- Autorización mediante `VerifiedPrincipal` y los seis permisos Java existentes:
  `cash-register.{read,create,update}` y `cash-shift.{read,open,close}`.
- Actor de apertura/cierre derivado de `principal.user_id()`, nunca del request.
- `BEGIN IMMEDIATE`, CAS scoped y `SqliteAuditWriter::append` en la única UoW.
- Acciones auditadas: `cash-register.created`, `cash-register.updated`,
  `cash-shift.opened`, `cash-shift.closed`.

La apertura exige sucursal y caja ACTIVE. El cierre no exige esos estados: un
principal explícito en `PrincipalScope::recovery` puede cerrar el turno ya abierto
tras desactivación, con `cash-shift.close`; el modo normal no puede usar esa ruta.

## 4. Paridad y diferencia pendiente

La estructura y reglas restauran V11/V12 y `CashRegisterService` /
`CashShiftService`. La diferencia deliberada es física: singular Local y decimales
como texto canónico, pendiente D-CASH. La coordinación Java `FOR UPDATE` se
sustituye por la única UoW SQLite `BEGIN IMMEDIATE`; la futura creación de venta
queda obligada a usar el puerto y la misma sección crítica.

No se implementaron ventas, pagos, movimientos de efectivo ni conciliación. La
tabla `candidate_open_sale_guard` vive exclusivamente dentro del fixture de tests
y demuestra la carrera cierre–venta; no es DDL de producto ni una representación
de venta.

## 5. Pruebas SQLite reales

`cash_store_tests` ejecuta nueve escenarios sobre un archivo temporal:

1. Caja → turno → cierre → auditoría USER → reapertura durable.
2. Unicidad de código por sucursal, FK cross-org y rechazo de cinco decimales.
3. Dos pools intentan abrir: exactamente uno confirma OPEN.
4. Cierre CAS, segundo cierre rechazado e historial CLOSED ilimitado.
5. Venta abierta bloquea cierre; fallo del writer revierte el cierre completo.
6. Carrera entre crear una venta de prueba y cerrar turno: solo venta+OPEN o
   cierre+sin venta puede confirmar.
7. Cierre de recuperación tras desactivar branch/caja; sesión revocada no vuelve a
   construir un principal.
8. Deny-by-default cross-org, límites de página y update de caja con CAS.
9. Cancelación de una UoW `BEGIN IMMEDIATE` tras insertar una caja: el rollback
   implícito ocurre antes de reutilizar la conexión y no queda una escritura parcial.

## 6. Validación ejecutada

| Control | Resultado real |
| --- | --- |
| `cargo fmt --all -- --check` | ✅ |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | ✅ |
| `cargo test --locked --workspace --all-features` | ✅ 255 pruebas, 0 fallos |
| `cargo build --locked --workspace --all-features` | ✅ |
| Prueba focalizada `cash_store_tests` | ✅ 9 pruebas SQLite reales |
| Linux/CI y Migration Lab PostgreSQL 17 | No ejecutados en este incremento Windows; la última evidencia Linux de 1Q sigue condicionada por el gate normativo |

Distribución observada: `migration-lab` 27 · `pos-core` 125 ·
`pos-credentials` 8 · `pos-local` 11 · `pos-storage-sqlite` 84. El incremento
suma nueve pruebas de persistencia candidata de caja/turno al total 1Q de 246.

## 7. Decisiones pendientes

| ID | Decisión requerida | Recomendación técnica, no aprobación |
| --- | --- | --- |
| D-CASH-01 | Nombres singulares y estructura de `cash_register` | Aprobar como adaptación Local coherente con `0001` |
| D-CASH-02 | `cash_shift`, FKs same-org y estados OPEN/CLOSED | Aprobar como restauración de V12 |
| D-CASH-03 | Importe Local `TEXT` canónico escala 4 | Aprobar: exactitud y rango sin `REAL` |
| D-CASH-04 | Índice parcial OPEN + sección crítica venta/cierre | Aprobar: es la barrera física/concurrente obligatoria |

Siguen pendientes D-AUTH-01..08, D-AUTH-10, D-AUTH-11, D-INST-01,
D-INST-02, ADR-0022 y D-CAT-01..03. D-AUTH-09 permanece bloqueada para parámetros
Argon2id comerciales. Nada de este incremento los aprueba.

## 8. Próximo incremento

Proponer la persistencia candidata de ventas reutilizando `pos-core::sales` y la
superclave `(cash_shift_id, organization_id, branch_id, cash_register_id)` ya
preparada por `0010`. Debe crear la venta OPEN en la misma UoW/serialización que el
cierre, antes de abordar checkout, pagos o decrementos de inventario.
