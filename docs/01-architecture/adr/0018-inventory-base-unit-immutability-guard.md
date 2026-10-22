# ADR-0018: Inventory Item base unit immutability guard

- **Status:** Accepted
- **Date:** 2026-10-15

## Context

ADR-0009 introduced `InventoryItem` with a mutable `baseUnit`, explicitly deferring
the decision of *when* it must stop changing: "Changing a base unit will need a
guarded contract before dependent inventory data is introduced." That data now
exists — branch balances and the append-only movement ledger (`V17`), Product
recipes and ModifierOption effects (`V18`), SaleItem consumption snapshots
(`V19`/`V21`) and purchase-order lines (`V23`). The quantity stored on each of
those rows has no unit of its own: its meaning is the item's `base_unit` at the
time it was captured. Changing `base_unit` afterwards silently reinterprets
historical quantities, recipes, balances and purchase orders.

The obvious implementation — "list dependencies; if none, update `base_unit`" — is
a time-of-check/time-of-use (TOCTOU) race: a recipe, movement or purchase-order
line can be inserted between the check and the update, and both transactions
commit, leaving a dependency interpreted against a stale unit. Most dependencies
are also *deletable* (recipe/modifier replacement, removing a DRAFT order line),
so "no rows today" does not mean "never used".

## Decision

1. `base_unit` may change **only while** the item has no operational facts or
   historical dependencies. When any exists, it is immutable.
2. `V24` adds `inventory_items.unit_locked BOOLEAN NOT NULL DEFAULT false` and
   backfills it to `true` for every item already referenced by `branch_inventory`,
   `inventory_movements`, `product_inventory_recipe_components`,
   `modifier_option_inventory_effects`, `sale_item_inventory_components` or
   `purchase_order_lines`. The marker is **persistent**: it is not cleared when a
   deletable dependency disappears, so historical use is never lost. A dynamic
   existence check alone would be insufficient for that reason.
3. `PATCH` with a different `baseUnit` on a locked item responds **409
   `INVENTORY_ITEM_UNIT_LOCKED`** and does not touch the resource. Sending the
   current unit remains an idempotent no-op (no version bump, no audit). Name,
   description, `fractionalUnitAllowed` and `status` remain editable; only the
   unit is frozen.
4. Concurrency is coordinated by a **pessimistic row lock on the `InventoryItem`**
   as the single serialization point, owned by `InventoryItemUnitGuard`:
   - Every dependency writer calls `lockAndMarkReferenced(organizationId, itemIds)`
     in its own transaction, **before** reading `base_unit` or inserting the
     dependency. The guard locks the items with `SELECT ... FOR UPDATE` in a
     deterministic ascending-id order and marks `unit_locked = true`.
   - The unit-change path locks the same row (`findByIdAndOrganizationIdForUpdate`)
     and rejects the change when the flag is set.
   - `unit_locked` is **monotonic**. The field is mapped `@Column(updatable = false)`,
     so a Hibernate entity flush never writes the column: a managed instance loaded
     with `unitLocked = false` before the mark cannot revert it even if the same
     transaction later modifies and flushes that instance. Only the guard's
     dedicated statement can set it. The unit-change check reads the committed flag
     through a scalar projection (`findUnitLockedFlag`) instead of the possibly stale
     managed instance, so a dependency created earlier in the same transaction still
     blocks the unit change.
   - Because both sides take the same row lock, either the dependency observes the
     already-changed unit, or the unit change is rejected; a stale-unit dependency
     can never coexist with a committed unit change. Bookkeeping writes
     (`base_unit`, `unit_locked`) bypass `@Version`; the guard's marker update is a
     bulk statement that does not bump the item version.
5. **Documented global lock order** to avoid deadlocks:
   `CashShift → PurchaseOrder → Supplier → InventoryItem → {BranchInventory,
   ProductInventoryRecipe, ModifierOptionInventoryConfig}`. Items are always locked
   by ascending `id`. In particular, checkout consumption locks the consumed items
   before the branch balances, matching manual stock mutations, so the
   `inventory_movements` foreign-key `FOR KEY SHARE` on the parent item cannot form
   a lock cycle with a concurrent manual adjustment.
6. **Scope of the guarantee.** This increment protects the base-unit invariant, i.e.
   a concurrent `base_unit` change versus dependency creation. The purchase-order
   *issue-time* TOCTOU (ADR-0017) was a separate, documented residual that a
   follow-up increment (Purchasing Issuance Consistency) now closes:
   - concurrent `base_unit` change — **protected here**: the guard serializes it with
     the purchase-order line and any other dependency;
   - concurrent deactivation of the `InventoryItem` (`status = INACTIVE`) — **now
     protected at issue time**: the emission locks the items by ascending `id` and
     reads their fresh state (ADR-0017 §4);
   - concurrent deactivation of the `Supplier` — **now protected at issue time**: the
     emission locks the supplier row and reads its fresh state (ADR-0017 §4).

## Consequences

- Historical quantities keep their original meaning for the lifetime of the item;
  recipes, sales, balances and purchase orders stay coherent.
- The frontend can read `unitLocked` on `InventoryItemResponse` to disable the unit
  selector and explain the `409 INVENTORY_ITEM_UNIT_LOCKED`.
- A few dependency writers (recipe, modifier effects, stock, sale consumption,
  purchase-order lines) now acquire an item row lock; this is a minimal, shared
  policy change with a single documented global lock order.
- `sale_item_inventory_components` (written by Sales during checkout) does not call
  the guard directly, but it only ever references items that already have a live
  recipe component, which permanently locked the unit. The invariant therefore holds
  without a Sales change.
- Purchase Receipts, when implemented, will also be a dependency: their lines and
  inbound movements must go through the same guard (or require the item to be
  already locked). No receipt tables are added here.
- Item updates that change the unit are serialized with dependency creation, which
  may briefly block under contention; accepted for correctness.
