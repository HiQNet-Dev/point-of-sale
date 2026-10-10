# Cash Registers — modelo de datos

**Implementado en `V11__create_cash_registers.sql`** (PR: Cash Registers).
Introduce `CashRegister`, propiedad de una **Branch** (una caja física pertenece
a una sucursal). Complementa `docs/02-database/domain-model.md`,
`docs/03-api/api-guidelines.md` y `docs/04-security/rbac.md`.

## Propósito

Una **cash register** ("caja") es el registro de una caja/terminal física de una
sucursal, base para el futuro módulo de turnos y ventas. Esta fundación define
el **registro** (identidad y estado), **no** el ciclo operativo: no modela turnos,
apertura/cierre, arqueos ni saldos.

Fuera de alcance de esta fundación: turnos (`CashShift`), saldos/apertura/cierre,
ventas/pagos/tickets, inventario, variantes de producto, hardware (impresora,
cajón, terminal, IP/MAC/serie).

## Modelo

```text
cash_registers
--------------
id              UUID     PK
organization_id UUID     -> organizations(id)
branch_id       UUID     -> branches(id)
code            VARCHAR(64)   slug normalizado (trim+lowercase), inmutable
name            VARCHAR(120)  trim;  CHECK length(trim(name)) > 0
status          VARCHAR(16)   ACTIVE | INACTIVE   (default ACTIVE)
version         BIGINT   @Version (optimistic locking)
created_at / updated_at TIMESTAMPTZ
UNIQUE (branch_id, code)        -- cash_registers_branch_code_unique
CHECK  code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'  -- cash_registers_code_format
FK (branch_id, organization_id) -> branches (id, organization_id)  -- cash_registers_branch_fk
```

Índice de lectura: `(branch_id, name)` en `cash_registers`.

`V11` reutiliza la superkey `branches_id_organization_unique UNIQUE (id,
organization_id)` creada en `V6` para la FK compuesta, garantizando que la caja y
su sucursal pertenecen a la **misma organización**.

## Decisiones clave

- **`code` inmutable** por scope: slug único **por Branch** (`UNIQUE (branch_id,
  code)`); dos sucursales distintas pueden tener una caja `caja-1`. Normalizado
  (`trim` + `lowercase`) y validado como slug (`^[a-z0-9]+(-[a-z0-9]+)*$`). No
  cambia tras crear (no se acepta en el `PATCH`).
- **`name` no único** (dos cajas "Caja Principal" son válidas); solo `code`
  identifica. No-blank, `trim`, máx 120.
- **`status` sin DELETE**: la baja es `status INACTIVE`; no hay borrado físico.
  Transición `ACTIVE ↔ INACTIVE` bajo optimistic locking. El `status` NO se fija
  en el `create` (siempre `ACTIVE`); solo el `PATCH` lo cambia.
- **Same-org en la base de datos**: la FK compuesta `(branch_id, organization_id)
  → branches(id, organization_id)` impide asociar una caja a una branch de otra
  organización. El `organization_id` se deriva de la branch, nunca del cliente.
- **Sin campos especulativos**: no se guardan saldos, montos, turnos ni datos de
  hardware. No se usa `double`/`float`/`BigDecimal`/dinero en esta fundación.

## Integridad (PostgreSQL es la autoridad)

| Violación | Cómo la impide PostgreSQL |
| --- | --- |
| Caja Org B + Branch Org A | FK compuesta `(branch_id, organization_id) → branches(id, organization_id)` |
| Código de caja duplicado (por branch) | `UNIQUE (branch_id, code)` |
| `code` no slug | `CHECK` de formato |
| `name` en blanco | `CHECK length(trim(name)) > 0` |

El pre-check de Java (`existsByBranchIdAndCode`) es UX; la carrera concurrente la
cierra el constraint traducido por `ConstraintTranslator`
(`cash_registers_branch_code_unique` → `409 CASH_REGISTER_CODE_ALREADY_EXISTS`).

## Autorización

Las cajas son **branch-scoped**: `cash-register.read`, `cash-register.create` y
`cash-register.update` se evalúan con `hasBranchPermission` (org-wide **o**
membership ACTIVE de esa branch), **nunca** promoviendo a org-scope. Los tres
permisos se siembran en `V11` (catálogo 32 → 35). Sin DELETE: `DELETE`/`PUT` no
se implementan y caen en `denyAll()`.

## Auditoría

Toda mutación crea un `AuditEvent` en la misma transacción (port
`shared.audit.AuditPort`, `Propagation.MANDATORY`): `cash-register.created` y
`cash-register.updated` (`entityType` = `cash_register`), branch-scoped, con
metadata `branchId` + `code` (create) y `changedFields` (update). Un `PATCH` sin
cambios efectivos es **no-op**: no muta, no incrementa versión ni emite evento.

## Endpoints

Véase `docs/03-api/api-guidelines.md` (§ Cash Registers): `POST/GET
/…/branches/{branchId}/cash-registers`, `GET/PATCH
/…/branches/{branchId}/cash-registers/{cashRegisterId}`.

## Frontera con turnos y ventas (futuro)

Una **cash register** NO es un **turno**. El turno (`CashShift`, apertura/cierre,
saldos) y las ventas/pagos/tickets siguen fuera de alcance y no se implementan
aquí.
