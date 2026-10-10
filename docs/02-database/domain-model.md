# Modelo de dominio conceptual

**Documento vivo.** `Organization`, `Branch`, `User` (identidad +
administración), `RefreshSession`, RBAC, memberships, auditoría, bootstrap y el
catálogo (`Category`/`Product` + modifiers + cash registers) ya están
implementados (migraciones `V1`–`V11`); el resto de las entidades descritas aquí
son futuras.

Versión actual: `0.2.0-SNAPSHOT`.

## Jerarquía principal

```text
Organization (organización / empresa)  ✔ implementado
 ├── User (usuario)                    ✔ implementado (identidad + administración; RBAC implementado)
 │    └── RefreshSession (familia de refresh tokens)  ✔ implementado
 ├── Role / RolePermission / UserOrganizationRole   ✔ implementado (V5)
 ├── AuditEvent (historial append-only)  ✔ implementado (V7)
 ├── Category / Product (catálogo)     ✔ implementado (V9)
 ├── ModifierGroup / ModifierOption (modifiers)  ✔ implementado (V10)
 │    └── ProductModifierGroup (producto ↔ grupo, ordenado)  ✔ implementado (V10)
 └── Branch (sucursal)                 ✔ implementado
      ├── BranchMembership (user ↔ branch, con roles branch-scoped)  ✔ implementado (V6)
      ├── CashRegister (caja)  ✔ implementado (V11)
      ├── Shift (turno) → Sale (venta) → Payment (pago)  (futuro)
      ├── Stock (existencias)  ← descuenta por venta, compra y mermas
      └── Inventory (ámbito de inventario de la sucursal)
```

## Entidades previstas (futuro)

| Entidad | Ámbito | Notas |
| --- | --- | --- |
| Organization | raíz | **implementada** — ver `docs/02-database/organization-branch-model.md` |
| Branch | organización | **implementada** — ver `docs/02-database/organization-branch-model.md` |
| User | organización | **implementada (identidad + administración)** — email normalizado único por organización (CHECK `V4`), `password_hash` (BCrypt; `VARCHAR(255)`), intentos/lockout, `status ACTIVE/INACTIVE` sin hard delete; RBAC futuro — ver `docs/02-database/users-model.md` |
| RefreshSession | usuario | **implementada** — refresh opaco rotativo; solo SHA-256 persistido; familia 30d absolutos; `refreshExpiresAt` = `min(expires_at, family_expires_at)` (`docs/04-security/authentication.md`) |
| Role | organización | **implementada (`V5`)** — `code` slug único por org e inmutable; `status ACTIVE/INACTIVE`; sin DELETE; `@Version` |
| Permission | sistema | **implementada (`V5`)** — catálogo global sembrado por Flyway; `code` PK; sin CRUD de cliente |
| RolePermission | — | **implementada (`V5`)** — grant `role_id → permission_code` (unique compuesto) |
| UserOrganizationRole | organización | **implementada (`V5`)** — user posee un rol para toda la org; FKs compuestas impiden cross-tenant |
| BranchMembership | organización+sucursal | **implementada (`V6`)** — user pertenece a una branch (máx. 1 por org+branch+user), `status ACTIVE/INACTIVE`, `@Version`; ver `docs/02-database/branch-memberships-model.md` |
| BranchMembershipRole | organización | **implementada (`V6`)** — roles branch-scoped de una membership; reutiliza las definiciones de `roles`; FKs compuestas impiden cross-tenant |
| AuditEvent | organización | **implementada (`V7`)** — historial append-only de mutaciones administrativas; trigger DB impide UPDATE/DELETE; actor USER/SYSTEM con FKs compuestas same-org; metadata JSONB acotada (≤16 KiB); ver `docs/02-database/audit-model.md` |
| Category | organización | **implementada (`V9`)** — agrupador de catálogo; `code` slug único por org e inmutable; `status ACTIVE/INACTIVE`; sin DELETE; ver `docs/02-database/catalog-model.md` |
| Product | organización | **implementada (`V9`)** — `sku` canónico (MAYÚSCULAS) inmutable y único por org, `barcode` opcional único cuando está presente, `unit_price NUMERIC(19,4)` (moneda heredada de la org), `category_id` opcional same-org; sin DELETE; ver `docs/02-database/catalog-model.md` |
| ModifierGroup | organización | **implementada (`V10`)** — grupo de modifiers reutilizable; `code` slug único por org e inmutable; bounds `min_selections`/`max_selections`; `required`/`singleChoice`/`multipleChoice` **derivados** (no persistidos); `status ACTIVE/INACTIVE`; sin DELETE; ver `docs/02-database/product-modifiers-model.md` |
| ModifierOption | organización | **implementada (`V10`)** — opción de un grupo; `code` slug inmutable único dentro del grupo; `price_delta NUMERIC(19,4)` **con signo**; `display_order`; `status ACTIVE/INACTIVE`; sin DELETE |
| ProductModifierGroup | organización | **implementada (`V10`)** — relación N—M producto↔grupo con `display_order`; PK `(product_id, modifier_group_id)`; FKs compuestas same-org; reemplazo atómico ordenado (`product.modifiers.manage`) |
| CashRegister | sucursal | **implementada (`V11`)** — caja física **branch-owned**; `code` slug único por branch e inmutable; `name` no único; `status ACTIVE/INACTIVE`; FK compuesta same-org `(branch_id, organization_id)`; sin DELETE; ver `docs/02-database/cash-register-model.md` |
| Shift | sucursal | Turnos y arqueos (futuro) |
| ProductVariant | organización | tamaños, sabores (futuro) |
| Ingredient | **organización** | Insumos del catálogo |
| Recipe | **organización** | Producto → ingredientes + cantidades |
| Stock | **sucursal** | Existencias por ingrediente/producto en cada branch |
| Sale / SaleLine / Payment | sucursal | Dinero siempre en `BigDecimal` |
| Supplier / PurchaseOrder | organización | Compras (futuro) |
| Expense | sucursal u organización | Gastos (futuro) |

## Regla clave (repetida)

**El inventario NO es global cuando hay varias sucursales: las existencias
pertenecen a la sucursal.** El catálogo (productos, ingredientes, variantes)
pertenece a la organización; el stock se consulta y modifica siempre por
`branch`. Nada de `branchId` fijo ni IDs mágicos (ver
`docs/05-development/coding-guidelines.md`).

**Aislamiento:** ningún recurso de una rama se resuelve por id genérico; toda
operación pasa por `organization_id` (`findByIdAndOrganizationId`).

## Convenciones de dinero y tiempo

- **Dinero:** `BigDecimal` siempre. Nunca `double`/`float`. Escala monetaria
  definida por columna (p. ej. `numeric(19,4)` a decidir en el ER final).
- **Moneda:** `currency` de la organización en ISO 4217 (validado con Java
  `Currency`); `char(3) → varchar(3)` + CHECK en DDL.
- **Timestamps:** `created_at` / `updated_at` en **UTC** (`TIMESTAMPTZ`,
  `Instant` en JPA) en todas las entidades; zona horaria del negocio
  configurable por organización (con herencia a branch).
- **Auditoría:** quién/cuándo/qué; operaciones de dinero inmutables
  (sin hard delete) — ver `docs/04-security/security-model.md`.

## Relaciones

- Organization 1—N Branch ✔; Organization 1—N User ✔; Organization 1—N Role ✔
  (`V5`); Organization 1—N Category/Product ✔ (`V9`); Organization 1—N
  Ingredient (futuro). Category 1—N Product (opcional) ✔ (`V9`).
- Organization 1—N ModifierGroup ✔ (`V10`); ModifierGroup 1—N ModifierOption ✔
  (`V10`); Product N—M ModifierGroup (vía `product_modifier_groups`, ordenada) ✔
  (`V10`).
- Branch 1—N CashRegister ✔ (`V11`).
- User 1—N RefreshSession ✔; User N—M Role (vía `user_organization_roles`, ✔).
- Branch 1—N BranchMembership ✔ (`V6`); BranchMembership N—M Role (vía
  `branch_membership_roles`, ✔ — roles del catálogo de `roles`, sin modelo
  BranchRole propio).
- Organization 1—N AuditEvent ✔ (`V7`); Branch 0—N AuditEvent (opcional);
  User 0—N AuditEvent (como actor).
- Branch 1—N Stock (futuro).
- Product 1—N Variant; Recipe N—M Ingredient (con cantidades) (futuro).
- Sale 1—N Payment; Sale 1—N SaleLine; Sale N—1 Branch / Sale N—1 CashRegister (`V11`) (futuro).
