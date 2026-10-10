# Cash Shifts — modelo de datos

**Implementado en `V12__create_cash_shifts.sql`** (PR: Cash Shifts Foundation).
Introduce `CashShift`, propiedad de un **CashRegister** de una **Branch**
(`Organization → Branch → CashRegister → CashShift`). Complementa
`docs/02-database/domain-model.md`, `docs/03-api/api-guidelines.md` y
`docs/04-security/rbac.md`.

## Propósito

Un **cash shift** ("turno de caja") es el registro operativo e histórico de una
sesión de caja: cuándo se abrió, con qué monto declarado, quién la abrió, cuándo
se cerró, con qué monto declarado y quién la cerró. Esta fundación define el
**ciclo de vida del turno** (apertura y cierre), **no** la conciliación: no
modela ventas, pagos, tickets, movimientos de efectivo ni el efectivo esperado.

Fuera de alcance de esta fundación: ventas/pagos/tickets, movimientos de caja
(`CashMovement`), efectivo esperado/diferencias/arqueos, inventario, variantes de
producto, hardware.

## Modelo

```text
cash_shifts
-----------
id                     UUID     PK
organization_id        UUID     -> organizations(id)
branch_id              UUID     -> branches(id)
cash_register_id       UUID     -> cash_registers(id)
opened_by_user_id      UUID     -> users(id)
closed_by_user_id      UUID     NULL -> users(id)
status                 VARCHAR(16)   OPEN | CLOSED
opening_amount         NUMERIC(19,4) CHECK >= 0
closing_declared_amount NUMERIC(19,4) NULL CHECK >= 0
opened_at              TIMESTAMPTZ
closed_at              TIMESTAMPTZ NULL
version                BIGINT   @Version (optimistic locking)
created_at / updated_at TIMESTAMPTZ
```

Constraints e índices:

```text
-- una sola apertura por caja (autoridad de la regla)
CREATE UNIQUE INDEX cash_shifts_one_open_per_register
    ON cash_shifts (cash_register_id) WHERE status = 'OPEN';

-- consistencia de estado OPEN vs CLOSED
CHECK cash_shifts_state_consistency
CHECK opening_amount_non_negative
CHECK closing_declared_amount_non_negative

-- tenant/same-org (FKs compuestas)
FK (cash_register_id, organization_id, branch_id)
   -> cash_registers (id, organization_id, branch_id)  -- cash_shifts_register_fk
FK (opened_by_user_id, organization_id)
   -> users (id, organization_id)                       -- cash_shifts_opened_by_fk
FK (closed_by_user_id, organization_id)
   -> users (id, organization_id)                       -- cash_shifts_closed_by_fk

-- lectura paged/por caja
INDEX (cash_register_id, opened_at DESC, id DESC)
```

`V12` añade la superkey `cash_registers_id_scope_unique UNIQUE (id,
organization_id, branch_id)` para poder anclar la FK del turno a la caja, la
organización y la branch a la vez (same-org **y** same-branch en la base de
datos).

## Ciclo de vida

```text
OPEN --close--> CLOSED
```

- Solo existe `OPEN → CLOSED`. **No** hay `CANCELLED/VOID/SUSPENDED/PAUSED` ni
  reapertura.
- Un turno `CLOSED` es **inmutable**: no se reabre, no se editan montos, no se
  borra. Cerrar un turno ya cerrado es un error (`409
  CASH_SHIFT_ALREADY_CLOSED`), nunca un no-op silencioso.

## Decisiones clave

- **Una sola apertura por caja**: mientras un turno esté `OPEN`, abrir otro sobre
  la misma caja es `409 CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT`. La autoridad final
  es el **índice único parcial** `cash_shifts_one_open_per_register` — nunca un
  `SELECT exists` en Java. Un turno `CLOSED` **no** bloquea una apertura nueva, por
  lo que una caja acumula historia ilimitada.
- **Actores derivados del servidor**: `opened_by_user_id`/`closed_by_user_id` se
  toman del principal autenticado (JWT), nunca del cuerpo. `openedAt`/`closedAt`
  los fija el servidor. El que cierra puede ser distinto del que abrió.
- **Dinero `BigDecimal` → `NUMERIC(19,4)`**: nunca `double`/`float`. Los montos son
  **declaraciones del operador**, no conciliaciones. `>= 0`, `scale <= 4`; un monto
  con más de 4 decimales se **rechaza** (nunca se redondea en silencio); los montos
  válidos se normalizan a escala 4. Moneda = `Organization.currency` (sin columna
  duplicada).
- **Estado coherente en la base**: `OPEN` ⇒ `closed_at`, `closed_by_user_id` y
  `closing_declared_amount` son `NULL`; `CLOSED` ⇒ los tres son `NOT NULL`
  (`cash_shifts_state_consistency`).
- **Same-org/same-branch en la base de datos**: las FKs compuestas impiden asociar
  un turno a una caja de otra organización o de otra branch, o a actores de otra
  organización.
- **`ACTIVE` solo al abrir**: la apertura exige `CashRegister.status = ACTIVE` y
  `Branch ACTIVE`. Un turno ya abierto **siempre puede cerrarse** aunque la caja o
  la branch se vuelvan `INACTIVE` después: el ciclo de vida de la branch/caja no se
  modifica desde este módulo.

## Integridad (PostgreSQL es la autoridad)

| Violación | Cómo la impide PostgreSQL |
| --- | --- |
| Segundo turno `OPEN` en la misma caja | Índice único parcial `cash_shifts_one_open_per_register` |
| Turno Org B + Caja Org A (u otra branch) | FK compuesta `(cash_register_id, organization_id, branch_id) → cash_registers(id, organization_id, branch_id)` |
| Actor de otra organización | FK compuestas `(…_user_id, organization_id) → users(id, organization_id)` |
| `CLOSED` sin datos de cierre / `OPEN` con datos de cierre | `CHECK cash_shifts_state_consistency` |
| Monto negativo | `CHECK opening_amount_non_negative` / `closing_declared_amount_non_negative` |

El pre-check de Java (`existsBy…Status(OPEN)`) es UX; la carrera concurrente la
cierra el índice único parcial traducido por `ConstraintTranslator`
(`cash_shifts_one_open_per_register` → `409
CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT`). El control optimista (`@Version` +
`expectedVersion`) protege el cierre.

## Autorización

Los turnos son **branch-scoped**: `cash-shift.read`, `cash-shift.open` y
`cash-shift.close` se evalúan con `hasBranchPermission` (org-wide **o** membership
ACTIVE de esa branch), **nunca** promoviendo a org-scope. Cerrar exige
`cash-shift.close` (no `cash-shift.open`). Los tres permisos se siembran en `V12`
(catálogo 35 → 38). Sin DELETE/PUT/PATCH: caen en `denyAll()`.

## Auditoría

Toda mutación crea **un** `AuditEvent` en la misma transacción (port
`shared.audit.AuditPort`, `Propagation.MANDATORY`; si el registro de auditoría
falla, la mutación se revierte): `cash-shift.opened` y `cash-shift.closed`
(`entityType` = `cash_shift`), branch-scoped, con actor `USER`. Metadata:
`branchId` + `cashRegisterId` + `openingAmount` (open, `.toPlainString()`) y
`cashRegisterId` + `declaredCashAmount` (close). No se guarda el request completo.

## Endpoints

Véase `docs/03-api/api-guidelines.md` (§ Cash Shifts): `POST/GET
/…/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts` y `GET/POST
…/cash-shifts/{cashShiftId}` (`/close`).

## Frontera con ventas y pagos (futuro)

Un **cash shift** NO es una **venta**. `Sale`/`SaleItem`, `Payment`, tickets,
`CashMovement` y el cálculo de efectivo esperado/diferencias siguen fuera de
alcance y no se implementan aquí. `findOpen` en el repositorio queda disponible
como punto de entrada para esos módulos, sin introducir lógica de ventas.
