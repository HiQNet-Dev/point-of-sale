# ADR-0010: Branch inventory balance and immutable ledger

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

V16 established Organization-owned reusable InventoryItem definitions. Physical stock is distinct operational state and belongs to a Branch. The POS needs auditable opening counts and manual corrections without coupling Inventory to Sales, Purchasing or Recipes.

## Decision

1. `BranchInventory` is a lazy materialized balance unique by Branch + InventoryItem. Missing rows read as exact zero/version zero; reads never create rows.
2. `InventoryMovement` is append-only in application and PostgreSQL. V17 supports only `OPENING` and signed nonzero `ADJUSTMENT`, storing the resulting balance on each movement.
3. Quantities use `NUMERIC(19,6)` and JSON strings, parsed as `BigDecimal` without rounding or floating point. Responses use six fixed decimal places.
4. Opening accepts nonnegative quantity including zero, only before any movement. A partial unique index reinforces one opening per branch/item. Manual adjustment requires a nonblank reason and may produce a negative balance; this administrative correction policy does not set future Sales checkout policy.
5. `UNIT` with `fractionalUnitAllowed=false` must remain integral. Fractional UNIT is opt-in; GRAM and MILLILITER support scale six.
6. Writers use PostgreSQL `INSERT ... ON CONFLICT DO NOTHING`, then `PESSIMISTIC_WRITE`, and require client `expectedVersion` as well as JPA `@Version`. Balance, movement and AuditEvent commit or roll back together.
7. Branch and item must be ACTIVE for writes. Historical reads remain available for inactive resources. Stock permissions are branch-scoped and distinct from item-definition permissions.

## Deferred

Recipes/BOM, modifier consumption, Sales checkout deduction, reservations, checkout negative-stock policy, Purchasing/receipts, Transfers, stocktakes, cost accounting and frontend UI are out of scope.
