# ADR-0013: Organization-scoped Negative Stock Policy

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

V16–V19 define organization-owned `InventoryItem`s, branch-owned stock with an append-only movement ledger, organization-owned recipes/modifier effects, and Sales-owned SaleItem consumption snapshots. A future checkout will consume tracked Inventory; whether that consumption may drive a branch balance negative is a commercial decision of the business, not of a branch, register, user or item. C3a records that decision as configuration only.

## Decision

1. `InventoryNegativeStockPolicy` is an Organization-owned enum with exactly two values: `STRICT` and `ALLOW_NEGATIVE`. `STRICT` means a future Sales checkout MUST fail when tracked Inventory is insufficient; `ALLOW_NEGATIVE` means a future Sales checkout MAY complete and the branch balance may become negative.
2. Ownership is the Organization. Branch, CashRegister, User, Product, InventoryItem and Role overrides are explicitly deferred; no per-branch policy exists in v0.3.
3. The default is `STRICT` for every new Organization (domain default plus the DB `DEFAULT`), and existing Organizations migrated by V20 become `STRICT`.
4. V20 adds `organizations.inventory_negative_stock_policy VARCHAR(16) NOT NULL DEFAULT 'STRICT'` with a `CHECK (IN ('STRICT','ALLOW_NEGATIVE'))`. V1–V19 remain immutable. The DB `DEFAULT` is kept as a defense for direct/administrative inserts, mirroring `organizations.status`.
5. The policy is configured only through the existing `PATCH /api/v1/organizations/{organizationId}` endpoint using the existing `organization.update` permission; the request field is optional and, when absent or `null`, the value is left unchanged (never reset to `STRICT`). Unknown enum values are rejected by the existing malformed-request contract (400 `MALFORMED_REQUEST`). No new endpoint, permission or JWT claim is introduced; the permission catalog stays at 51.
6. The change is a normal partial Organization update: `expectedVersion` optimistic lock, `version + 1` on a real change, no version bump or AuditEvent on a same-value no-op, and one transactional `organization.updated` AuditEvent whose `changedFields` includes `inventoryNegativeStockPolicy`. An audit failure rolls back both the policy and the version.
7. Manual Adjustment and Opening Stock are unaffected: they continue to allow a negative balance per their existing contract. The policy is not reused in `InventoryStockService`.

## Deferred

Sales checkout consumption of the policy, `SALE_CONSUMPTION` movements, stock deduction, reservations, per-branch overrides, a `WARN`/soft mode, and any Inventory/Admin frontend remain separate work (C3b and later).
