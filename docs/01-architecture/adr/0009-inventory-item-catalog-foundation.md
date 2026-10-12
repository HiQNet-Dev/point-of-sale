# ADR-0009: Inventory Item catalog foundation

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

Inventory v0.3 begins with reusable item definitions, before branch stock, movements, recipes or Sales integration. Product ingredients are not the only tracked materials: packaging, consumables and retail materials must also fit the model. Existing POS conventions assign organization-wide configuration to Organization and physical operations to Branch.

## Decision

1. `InventoryItem` is an **Organization-owned** definition. It has no Branch, quantity, stock or balance fields. Future branch balances and movements belong to Branch scope.
2. `code` is an immutable, normalized lowercase slug unique per Organization. Name, optional description, `baseUnit`, `fractionalUnitAllowed` and `ACTIVE`/`INACTIVE` status are mutable through optimistic concurrency (`expectedVersion` + `@Version`). Items are not hard-deleted.
3. Initial base units are `UNIT`, `GRAM` and `MILLILITER`, persisted as `VARCHAR` with a database `CHECK`, not a PostgreSQL enum. Quantities and conversions are not part of this slice. `fractionalUnitAllowed` defaults to false and is metadata only at this stage.
4. `baseUnit` is mutable while no recipe, balance or movement can refer to the item. A later migration/slice must prevent unit changes once such dependent facts exist; this slice does not add speculative dependency checks.
5. Two organization-scope permissions are added: `inventory.item.read` and `inventory.item.write`. Migration adds catalog entries but does not grant them to existing roles. Secure bootstrap grants all permissions present for a fresh installation.
6. Create/update are audited in the same transaction. Reads are not audited.

## Deliberately deferred

Branch stock, balances and opening/adjustments are specified by ADR-0010 / V17. Recipes/BOM, modifier consumption, Sales snapshots and checkout deduction remain later slices. Existing Products remain sellable without inventory configuration. Checkout negative-stock policy remains open until Sales integration.

## Consequences

- Catalog definitions can be shared across Branches while later stock remains branch-owned.
- V15 installations receive two permissions but no existing role silently gains them.
- Changing a base unit will need a guarded contract before dependent inventory data is introduced.
- This ADR does not authorize any stock mutation or alter POS checkout behavior.
