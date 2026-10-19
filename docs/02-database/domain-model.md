# Modelo de dominio conceptual

**Documento vivo.** `Organization`, `Branch`, `User` (identidad +
administración), `RefreshSession`, RBAC, memberships, auditoría, bootstrap, el
catálogo POS, Sales Foundation y Payments + Atomic Checkout están implementados
(V1–V15). `InventoryItem` organization-owned (V16), Branch stock y movements
(V17), recipes y modifier effects (V18) y snapshots + consumo de checkout
(V19–V21) están implementados. Purchasing está implementado (V22–V25):
`Supplier` organization-owned (V22), órdenes de compra con sus partidas (V23),
la guarda de inmutabilidad de la unidad base del `InventoryItem` (V24) y las
recepciones de mercancía con ledger `PURCHASE_RECEIPT` (V25).

Versión de esta release: `0.4.0`.

## Jerarquía principal

```text
Organization (organización / empresa)  ✔ implementado
 ├── User (usuario)                    ✔ implementado (identidad + administración; RBAC implementado)
 │    └── RefreshSession (familia de refresh tokens)  ✔ implementado
 ├── Role / RolePermission / UserOrganizationRole   ✔ implementado (V5)
 ├── AuditEvent (historial append-only)  ✔ implementado (V7)
 ├── Category / Product (catálogo)     ✔ implementado (V9)
 ├── InventoryItem (definición)         ✔ implementado (V16), organization-owned
 ├── Supplier (proveedor)               ✔ implementado (V22), organization-owned
 ├── PurchaseOrder (orden de compra)    ✔ implementado (V23), organization-owned + Branch destino
 │    └── PurchaseOrderLine (partida)   ✔ implementado (V23); referencia InventoryItem
 ├── ModifierGroup / ModifierOption (modifiers)  ✔ implementado (V10)
 │    └── ProductModifierGroup (producto ↔ grupo, ordenado)  ✔ implementado (V10)
 └── Branch (sucursal)                 ✔ implementado
      ├── BranchMembership (user ↔ branch, con roles branch-scoped)  ✔ implementado (V6)
      ├── CashRegister (caja)  ✔ implementado (V11)
      │    └── CashShift (turno de caja)  ✔ implementado (V12)
      │         └── Sale → SaleItem → SaleItemModifier  ✔ Sales Foundation (V13; OPEN→CANCELLED/COMPLETED)
      │              └── Payment[]  ✔ Atomic Checkout (V14; immutable CASH/CARD)
      └── BranchInventory / movements   ✔ implementado (V17); existencias físicas por Branch
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
| InventoryItem | organización | **implementada (`V16`)** — code slug inmutable y único por organización; baseUnit `UNIT`/`GRAM`/`MILLILITER`; sin stock ni cantidades; ver `docs/02-database/inventory-item-model.md` |
| Product | organización | **implementada (`V9`)** — `sku` canónico (MAYÚSCULAS) inmutable y único por org, `barcode` opcional único cuando está presente, `unit_price NUMERIC(19,4)` (moneda heredada de la org), `category_id` opcional same-org; sin DELETE; ver `docs/02-database/catalog-model.md` |
| ModifierGroup | organización | **implementada (`V10`)** — grupo de modifiers reutilizable; `code` slug único por org e inmutable; bounds `min_selections`/`max_selections`; `required`/`singleChoice`/`multipleChoice` **derivados** (no persistidos); `status ACTIVE/INACTIVE`; sin DELETE; ver `docs/02-database/product-modifiers-model.md` |
| ModifierOption | organización | **implementada (`V10`)** — opción de un grupo; `code` slug inmutable único dentro del grupo; `price_delta NUMERIC(19,4)` **con signo**; `display_order`; `status ACTIVE/INACTIVE`; sin DELETE |
| ProductModifierGroup | organización | **implementada (`V10`)** — relación N—M producto↔grupo con `display_order`; PK `(product_id, modifier_group_id)`; FKs compuestas same-org; reemplazo atómico ordenado (`product.modifiers.manage`) |
| CashRegister | sucursal | **implementada (`V11`)** — caja física **branch-owned**; `code` slug único por branch e inmutable; `name` no único; `status ACTIVE/INACTIVE`; FK compuesta same-org `(branch_id, organization_id)`; sin DELETE; ver `docs/02-database/cash-register-model.md` |
| CashShift | sucursal/caja | **implementada (`V12`)** — turno **register-owned**; ciclo `OPEN → CLOSED`, actores/timestamps del servidor, montos declarados `NUMERIC(19,4)` no negativos, un solo `OPEN` por caja (índice único parcial), FKs compuestas same-org/same-branch; sin DELETE; ver `docs/02-database/cash-shift-model.md` |
| Sale / SaleItem / SaleItemModifier | turno/caja/sucursal | **implementada (`V13`/`V14`)** — Sale scoped a Organization/Branch/Register/Shift, lifecycle `OPEN → CANCELLED` o `OPEN → COMPLETED`, total exacto de líneas ACTIVE, snapshots de catálogo/modifiers y sin hard delete; ver `docs/02-database/sales-model.md` |
| Payment | sale/turno/caja/sucursal | **implementado (`V14`)** — hecho inmutable CASH/CARD creado solo por checkout atómico; ver `docs/02-database/payment-model.md` |
| ProductVariant | organización | tamaños, sabores (futuro) |
| Ingredient | — | No es un aggregate separado; ingredientes y consumibles se modelan como `InventoryItem` |
| Recipe | **organización** | Producto → ingredientes + cantidades |
| Stock | **sucursal** | Existencias por ingrediente/producto en cada branch |
| Sale / SaleLine / Payment | sucursal | Dinero siempre en `BigDecimal` |
| Supplier | organización | **implementado (`V22`)** — datos maestros organization-owned; `code` slug único por org e inmutable; contacto/razón social/identificador fiscal opcionales; `status ACTIVE/INACTIVE` sin DELETE; `@Version`; ver `docs/02-database/purchasing-suppliers-model.md` |
| PurchaseOrder | organización+sucursal | **implementada (`V23`)** — cabecera con `branch_id` de destino, `code` slug único por org e inmutable, moneda copiada de la organización, snapshots del proveedor, estados DRAFT/ORDERED/PARTIALLY_RECEIVED/RECEIVED/CANCELLED (`V25` añade los dos de recepción) y `@Version` del agregado; ver `docs/02-database/purchase-orders-model.md` |
| PurchaseOrderLine | organización | **implementada (`V23`)** — partida con snapshots inmutables de `code`/`name`/`baseUnit` del `InventoryItem`, `line_number` único por orden, `quantity NUMERIC(19,6) > 0` y `unit_cost NUMERIC(19,4) >= 0` opcional |
| PurchaseReceipt | organización+sucursal | **implementada (`V25`)** — recepción inmutable de una orden emitida, con `idempotency_key` única por organización+orden y `payload_fingerprint`; ver `docs/02-database/purchase-receipts-model.md` |
| PurchaseReceiptLine | organización | **implementada (`V25`)** — línea con `quantity_received NUMERIC(19,6) > 0` en la unidad base de la partida; origina un movimiento `PURCHASE_RECEIPT` |
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
- Organization 1—N Supplier ✔ (`V22`); un mismo proveedor puede ser referenciado
  por órdenes de cualquier Branch de la organización.
- Organization 1—N PurchaseOrder ✔ (`V23`); PurchaseOrder N—1 Branch (destino) y
  N—1 Supplier; PurchaseOrder 1—N PurchaseOrderLine, que referencia un
  InventoryItem organization-owned y congela sus snapshots.
- Branch 1—N CashRegister ✔ (`V11`); CashRegister 1—N CashShift ✔ (`V12`).
- User 1—N RefreshSession ✔; User N—M Role (vía `user_organization_roles`, ✔).
- Branch 1—N BranchMembership ✔ (`V6`); BranchMembership N—M Role (vía
  `branch_membership_roles`, ✔ — roles del catálogo de `roles`, sin modelo
  BranchRole propio).
- Organization 1—N AuditEvent ✔ (`V7`); Branch 0—N AuditEvent (opcional);
  User 0—N AuditEvent (como actor).
- Branch 1—N BranchInventory (stock operativo) ✔ (`V17`).
- Product 1—N Variant (futuro); Product 1—N InventoryRecipe (componentes con cantidades) ✔ (`V18`).
- Sale 1—N Payment ✔ (`V14`); Sale 1—N SaleItem ✔ (`V13`); Sale N—1 Branch / Sale N—1 CashRegister ✔ (`V13`).
