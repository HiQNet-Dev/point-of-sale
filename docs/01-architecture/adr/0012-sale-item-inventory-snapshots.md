# ADR-0012: Historical Inventory consumption snapshots on SaleItems

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

Inventory owns the current Product recipe, ModifierOption effects and their exact per-product resolver. Sales owns a particular line's quote and lifecycle. A future checkout must consume the configuration that was quoted for that line, not re-read mutable Inventory configuration.

## Decision

1. `SaleItemInventoryComponent` is a Sales-owned child of `SaleItem`, persisted in V19. It stores the InventoryItem identity, code/name/base-unit snapshots, exact `quantity_per_unit` and exact `quantity_total` as `NUMERIC(19,6)`. Each line has at most one row per InventoryItem.
2. Sales invokes the existing Inventory resolver only after its own Product and modifier-selection validation. Inventory continues to resolve per-unit consumption; Sales multiplies by the SaleItem's existing positive integer quantity using `BigDecimal`. No Recipe/effect arithmetic is duplicated in Sales.
3. Add and material line edits replace the snapshot in the same transaction as Sale, SaleItem, commercial modifier snapshots and the existing Sales AuditEvent. A failed resolver, persistence step or audit rolls back the whole command. A semantic no-op returns before snapshot replacement. Logical removal, cancellation and completion retain the historical components.
4. A Product with no active recipe remains sellable and produces no snapshot rows. An active but invalid Inventory configuration fails the SaleItem mutation with the resolver's controlled configuration error. Cashiers need only their existing Sales permissions; the internal resolver does not add Inventory administration permissions.
5. Configuration edits, InventoryItem rename/status changes, and recipe deactivation do not rewrite existing snapshot rows. A subsequent material SaleItem edit follows the existing Sales requote behavior and resolves current configuration. Checkout does not read or deduct this snapshot in C2.
6. V19 does not backfill existing SaleItems. Historical and already-open pre-V19 lines remain valid with no snapshot rows and are treated as untracked until an explicit material edit/requote. Terminal Sales are unchanged.
7. Composite foreign keys preserve same-organization ownership for SaleItem and InventoryItem. Snapshot rows reference no Recipe or modifier-config version; the frozen values themselves are authoritative historical facts.

## Deferred

Checkout stock deduction, `SALE_CONSUMPTION` movements, inventory availability/reservation, negative-stock policy, Purchasing, Transfers, Costing and Inventory frontend remain separate work. No new endpoint or permission is introduced.
