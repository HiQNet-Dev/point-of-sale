# ADR-0011: Organization-owned Inventory recipes and modifier effects

- **Status:** Candidate — PR open
- **Date:** 2026-10-12

## Context

V16 defines organization-owned `InventoryItem`s and V17 defines branch-owned stock and its append-only movement ledger. The next Inventory slice needs a stable definition of theoretical consumption without coupling Catalog back to Inventory or changing Sales/Checkout behavior.

## Decision

1. Inventory owns `ProductInventoryRecipe` and its components, while Catalog continues to own `Product` and `ModifierOption`. Recipe and modifier configuration are organization-owned; Branch and stock are not part of these definitions. Composite foreign keys enforce same-organization references.
2. A Product without a recipe, or with an `INACTIVE` recipe, is untracked by the consumption resolver. An `ACTIVE` recipe must contain at least one positive component. Recipes are replace-all and cannot be deleted through the API. Inactive Products and ModifierOptions remain administrable.
3. Recipe components are unique per InventoryItem and use exact `NUMERIC(19,6)` quantities. Modifier effects are independently versioned per ModifierOption, unique per InventoryItem, signed, nonzero deltas; an empty effect set is valid. Discrete `UNIT` items reject fractional configuration.
4. HTTP quantities are decimal strings parsed to `BigDecimal`; numeric JSON tokens, excessive scale/precision and floating-point conversion are rejected. Component ordering has no business meaning, so writes compare as an item-keyed set and reads use stable InventoryItem ID order.
5. Both replace-all resources require `expectedVersion`, use a database row lock plus JPA `@Version`, and treat an equal effective replacement as a no-op (no version increment or audit). Parent, child rows and one logical AuditEvent are committed atomically.
6. `InventoryConsumptionResolver` combines the active recipe with only caller-validated selected ModifierOption IDs. It aggregates exact deltas by InventoryItem, removes zero totals, and fails with a controlled configuration-unavailable error for inactive references, negative totals, unsupported precision or fractional discrete UNIT totals. It never reads or mutates branch stock.
7. Permissions `inventory.recipe.read` and `inventory.recipe.write` are organization-scoped. V18 adds them to the catalog without granting them to existing roles; fresh bootstrap uses the complete catalog.

## Deferred

SaleItem consumption snapshots, Sales or Checkout integration, stock deduction/reservation, stock-based availability, Purchasing, Transfers, Costing and Inventory frontend remain separate work. No V18 schema references SaleItem.
