# ADR-0014: Atomic Checkout Inventory Consumption

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

V19 makes Sales the owner of the authoritative per-line Inventory consumption
snapshots (`sale_item_inventory_components`) resolved when an active `SaleItem`
is created or materially requoted. V20 makes the Organization owner of the
`inventoryNegativeStockPolicy` (`STRICT` | `ALLOW_NEGATIVE`, default `STRICT`).
V17 provides branch stock balances and an append-only movement ledger. Until
now nothing consumed these snapshots: checkout completed a Sale without touching
stock (`C3a` was configuration only). C3b closes that gap.

## Decision

1. Every `COMPLETED` Sale consumes tracked branch Inventory from the
   authoritative per-line snapshots of its `ACTIVE` lines, aggregated per
   `inventoryItemId`. Snapshots are the only source: the current Recipe, current
   ModifierOption effects and the current `InventoryItem.status` are never
   re-read at checkout. A line with zero snapshots (untracked product or a
   pre-V19 legacy line) contributes nothing and is not an error. Two snapshots
   of one item that disagree on `baseUnitSnapshot` fail with
   `INVENTORY_SNAPSHOT_INCONSISTENT` (409). Aggregation and any overflow are
   validated exactly (`NUMERIC(19,6)`); overflow fails with
   `INVALID_INVENTORY_QUANTITY` (400).

2. Policy is enforced per Organization. `STRICT` computes every shortage across
   the whole sale first and fails the entire checkout with `INSUFFICIENT_INVENTORY`
   (409) before any balance, movement, Sale or payment is written: STRICT
   consumption is all-or-nothing. `ALLOW_NEGATIVE` skips the shortage check and
   may drive a balance below zero. Shortage details are safe per-item strings
   (`code=… required=… available=…`) emitted in the canonical locked order.

3. Consumption writes exactly one `SALE_CONSUMPTION` movement per
   (`sale`, `InventoryItem`) with a negative `quantity_delta` and
   `resulting_quantity = prior + delta`; `reason` is null. V21 adds
   `inventory_movements.sale_id`, a composite FK to a new
   `sales (id, organization_id, branch_id)` superkey, re-declares the movement
   type/values checks to admit `SALE_CONSUMPTION`, and adds a partial unique
   index enforcing one consumption movement per sale and item. V1–V20 remain
   immutable. `insertBatch` appends the whole sale's movements in one round trip.

4. Locking is deterministic. Balances are bulk-ensured (`on conflict do
   nothing`) and then bulk-locked with `PESSIMISTIC_WRITE` ordered by
   `inventory_item_id` (PostgreSQL ordering is authoritative; the application
   never re-sorts). The global lock order is CashShift → BranchInventory → Sale
   optimistic claim, so a checkout that consumes stock can never deadlock
   against another checkout or against `CashShift.close`.

5. Ordering deviation (explicit). Consumption runs after payment construction
   and total validation but before `sale.complete`, `saveAndFlush` and
   `paymentRepository.saveAll`. The literal spec order (mutate stock after
   `sale.complete`) conflicts with the item-4 lock order, which requires the
   `BranchInventory` rows to lock before the Sale version claim. This deviation
   is deliberate and preserves every safety property that matters: payments are
   validated in memory first, and under `STRICT` a shortage aborts before any
   Sale or payment row persists. Payments are only persisted after the Sale
   aggregate is claimed, so a losing concurrent checkout still fails through the
   documented optimistic Sale conflict.

6. Error surface. `INSUFFICIENT_INVENTORY` (409, optional safe `details`) and
   `INVENTORY_SNAPSHOT_INCONSISTENT` (409) extend the existing `ApiError`
   contract; `BusinessException` gained an optional immutable `details` list and
   the handlers pass it through. The checkout dialog maps
   `INSUFFICIENT_INVENTORY` to an inline, retryable message and keeps the Sale
   and dialog open (it is not a recoverable refresh-and-retry conflict).

7. No new permission, JWT claim or `SecurityConfig` rule is introduced: a
   Cashier needs only `sale.checkout`. The permission catalog stays at 51 and
   existing grants are unchanged.

## Consequences

- Checkout is atomic across Sale completion, payments, stock balances and the
  movement ledger; a partial commit cannot occur.
- Two concurrent checkouts against the same scarce stock serialize on the
  balance row and exactly one succeeds under `STRICT`; the loser gets a truthful
  409.
- Manual Opening Stock and Adjustments are unchanged and remain unrestricted by
  the policy.

## Deferred

Returns/reversal and consumption compensation, reservations, per-branch policy
overrides, a soft `WARN` mode, costing/valuation, Purchasing/Transfers, and any
Inventory admin frontend remain separate work.
