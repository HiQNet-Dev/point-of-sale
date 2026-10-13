# Sales Foundation

`V13__create_sales_foundation.sql` introduces the operational transaction model:

```text
Organization → Branch → CashRegister → CashShift → Sale → SaleItem → SaleItemModifier
```

A Sale stores the exact organization, branch, register and shift scope and is created by a same-organization user. Composite foreign keys are the database barrier against cross-tenant, cross-branch and cross-register links.

## Lifecycle and aggregate

`V14` extends the lifecycle to `OPEN → CANCELLED` or `OPEN → COMPLETED`. `CANCELLED` and `COMPLETED` are immutable historical states. Completion is only possible through the atomic checkout boundary, which persists immutable Payment facts with exact coverage; see `docs/02-database/payment-model.md`. There is still no receipt number, tax, discount, inventory deduction or cash movement.

`Sale.version` is the optimistic concurrency boundary for add/update/remove/cancel. Mutations require the `expectedSaleVersion` or `expectedVersion` read by the client. A semantic update that keeps the same quantity and selected modifier options is a no-op: it neither bumps the version nor emits audit.

Lines are never physically deleted. `SaleItem` has `ACTIVE`/`REMOVED`; only active line totals are summed into `Sale.totalAmount`. Removed and cancelled lines retain their product and modifier snapshots.

## Inventory consumption snapshot — V19

Sales owns `sale_item_inventory_components`, a historical child of each `SaleItem`. On create and material re-quote, Sales calls the Inventory-owned resolver after validating the selected modifiers. It stores one aggregated row per InventoryItem with the ID, code/name/base-unit snapshots, exact `quantity_per_unit` and `quantity_total` (`NUMERIC(19,6)`). Sales multiplies resolver output by the existing integer SaleItem quantity using `BigDecimal`; no Recipe arithmetic is copied into Sales.

The snapshot is not exposed by the POS API. Existing SaleItem no-op, logical removal, cancellation and completion semantics remain unchanged; a no-op does not replace children, and removed/terminal lines retain their snapshot. Inventory configuration/metadata changes do not rewrite persisted rows. A material edit re-quotes under the existing Sales contract and uses current Inventory configuration. Missing/inactive Recipe means untracked; an invalid active configuration rejects the line mutation atomically. Cashiers continue to need only Sales permissions.

V19 performs no backfill: pre-V19 SaleItems, including OPEN lines, remain valid with no snapshot rows. Atomic checkout consumption (`V21`) deducts tracked branch Inventory from these snapshots and writes one `SALE_CONSUMPTION` movement per sale/item; whether consumption may drive a balance negative is governed by the organization-scoped negative stock policy.

## Pricing and snapshots

The client submits only `productId`, positive integer `quantity` (maximum 1,000,000), and selected `modifierOptionIds`. It cannot submit prices, totals, names, status or actors. The server reloads current catalog configuration and snapshots SKU, product name, base unit price, modifier group/option IDs, codes, names and signed deltas.

All monetary values use `BigDecimal` and `NUMERIC(19,4)`. Values are never calculated with binary floating point or silently rounded. The server computes:

```text
modifierUnitDelta = sum(selected option priceDelta)
effectiveUnitPrice = product unitPrice + modifierUnitDelta
lineTotal = effectiveUnitPrice × quantity
totalAmount = sum(ACTIVE lineTotal)
```

A negative effective unit price or a result outside `NUMERIC(19,4)` is rejected.

Product `ACTIVE` is the sellability authority. Category status does not implicitly make a product unavailable. Selected options must be active, belong to active groups assigned to the product, have no duplicates, and satisfy every assigned active group's min/max bounds. An inactive required group makes the product configuration unavailable; an inactive optional group can be omitted but not selected.

## Cash-shift coordination

Sale creation and checkout lock the exact `cash_shifts` row with `PESSIMISTIC_WRITE`. `CashShift.close` acquires the same lock and checks a narrow `OpenSalesForCashShiftPort`. This serializes create/checkout-versus-close so a CLOSED shift can never contain an OPEN Sale. Ordinary Sale edits stay optimistic; they do not acquire the coordination lock. An OPEN sale blocks shift close; cancellation is allowed even if branch/register were deactivated so recovery can finish.

## Authorization and audit

All Sale permissions are branch-scoped: `sale.read`, `sale.create`, `sale.update`, `sale.cancel`. Reads and cancellation use the branch-status-agnostic recovery authorization path; create and item mutations require operational parents ACTIVE. Every successful mutation records exactly one transactional audit event: `sale.created`, `sale.item.added`, `sale.item.updated`, `sale.item.removed`, or `sale.cancelled`.
