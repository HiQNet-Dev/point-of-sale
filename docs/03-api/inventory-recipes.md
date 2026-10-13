# Inventory recipe and modifier effects API — V18

These organization-scoped endpoints configure theoretical Inventory consumption. They do not read stock, expose branch IDs, alter Sales, or deduct/reserve quantities.

## Product recipe

```text
GET /api/v1/organizations/{organizationId}/products/{productId}/inventory-recipe
PUT /api/v1/organizations/{organizationId}/products/{productId}/inventory-recipe
```

GET returns the stored recipe or `404 INVENTORY_RECIPE_NOT_FOUND`. A Product with no recipe is untracked by Inventory. PUT creates or replaces the complete recipe; first creation uses `expectedVersion: 0`, then returns version `1`.

```json
{
  "expectedVersion": 1,
  "status": "ACTIVE",
  "components": [
    { "inventoryItemId": "<uuid>", "quantity": "18.000000" }
  ]
}
```

Components are a set, not an ordered list. An ACTIVE recipe needs at least one component. Each component InventoryItem may appear once and quantity must be positive. `INACTIVE` recipes remain readable and resolve as untracked. Inactive Products can be configured. New/changed components must use ACTIVE InventoryItems; unchanged inactive references can be retained while deactivating/recovering configuration.

## ModifierOption Inventory effects

```text
GET /api/v1/organizations/{organizationId}/modifier-options/{modifierOptionId}/inventory-effects
PUT /api/v1/organizations/{organizationId}/modifier-options/{modifierOptionId}/inventory-effects
```

GET returns stored configuration or `404 INVENTORY_MODIFIER_CONFIG_NOT_FOUND`. PUT replaces all effects; first creation uses `expectedVersion: 0`. An empty list is valid and represents no Inventory effect.

```json
{
  "expectedVersion": 0,
  "effects": [
    { "inventoryItemId": "<uuid>", "quantityDelta": "-250.000000" },
    { "inventoryItemId": "<other-uuid>", "quantityDelta": "250.000000" }
  ]
}
```

An effect is signed and nonzero; one InventoryItem may appear once. Recipe quantities and effects are decimal **strings**, parsed exactly as `BigDecimal`, stored as `NUMERIC(19,6)`, and returned with six decimal places. JSON numeric tokens, scientific notation, excess scale/precision and fractional quantities for discrete `UNIT` items are rejected.

## Concurrency and audit

Both PUT operations require the current `expectedVersion`. A stale value returns `409 CONCURRENT_MODIFICATION`. An effective no-op preserves version and emits no AuditEvent. A real replace, parent version increment, child rows and one logical AuditEvent are atomic. Reads are not audited.

`inventory.recipe.read` authorizes both GET routes and `inventory.recipe.write` authorizes both PUT routes at Organization scope. A role must receive grants explicitly; V18 does not grant these permissions to existing roles. Fresh bootstrap includes all 51 catalog permissions.

## Resolver boundary

The internal resolver combines an ACTIVE Product recipe with only caller-validated selected ModifierOption IDs, aggregates exact quantities by InventoryItem, removes zero totals and fails with `INVENTORY_CONFIGURATION_UNAVAILABLE` for inactive references or invalid final totals. Product with no recipe or an INACTIVE recipe is untracked. No public preview endpoint or SaleItem snapshot is introduced in this slice.
