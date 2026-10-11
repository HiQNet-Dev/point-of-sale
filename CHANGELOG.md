# Changelog

All notable changes to HiQNet POS are documented in this file.

## [Unreleased]

### POS Core

Development for v0.2.x has started.

- Added candidate Product Browser + Persisted Cart frontend: `/pos` lists and resumes OPEN Sales, creates a Sale only from an explicit action, browses the branch-context Operational Catalog, and performs SaleItem add/edit/remove/cancel mutations. The Sale detail is the persisted cart and pricing authority; no browser cart persistence, checkout UI or payment UI is included. Backend contracts and V1–V15 are unchanged.
- Added candidate Operational POS Catalog (`V15`): branch-context `pos.catalog.read`
  exposes organization-owned categories, ACTIVE products and one-read assigned
  modifier configuration without granting administrative catalog permissions.
  V1–V14 remain immutable; no branch-specific prices, inventory, or frontend UI.
- Corrected modifier-option API documentation: its current administrative
  collection accepts pagination only; it does not accept a `status` filter.
- Added Payments + Atomic Checkout (`V14`): Sale lifecycle now supports terminal `COMPLETED`, and immutable CASH/CARD payment facts are created only by one atomic checkout transaction. Exact BigDecimal coverage, split tender, cash tender/change, Sale aggregate concurrency, CashShift close coordination, `sale.checkout`/`payment.read` permissions (catalog 42 → 44), and transactional `sale.completed` audit are included. Partial payments, refunds, gateways, CashMovements, Inventory, Tickets and frontend checkout remain out of scope.

- Added Sales Foundation (`V13`): scoped `Sale` with an `OPEN → CANCELLED` lifecycle, retained `SaleItem`/modifier snapshots, server-authoritative exact pricing, sale aggregate optimistic concurrency, transactional audit, and branch-scoped `sale.read/create/update/cancel` permissions (catalog 38 → 42). An OPEN sale blocks CashShift close; sale creation and close coordinate through a scoped CashShift row lock.

- Added candidate POS operational session UI: `/pos` rediscovers the signed-in
  user's branches and their cash registers, recovers an OPEN cash shift, and
  supports opening or closing a shift with scoped queries and exact decimal
  strings. Inactive branches/registers remain discoverable for close recovery;
  selections and amounts are not persisted. Sales and checkout are not included.
- Added frontend authentication/session infrastructure: a memory-only
  authenticated API client, single-flight rotating refresh, `/auth/me` bootstrap,
  protected routing, and login/logout.
- Added the Catalog Foundation: organization-owned Categories and Products
  with an immutable normalized `code`/`sku`, an optional per-organization
  unique `barcode`, a base `unitPrice` (currency inherited from the
  organization) and optional same-organization category assignment (`V9`).
- Added the `category.*` and `product.*` permissions (permission catalog
  19 → 25).
- Added the Product Modifiers foundation: organization-owned, reusable
  Modifier Groups and Modifier Options (immutable slug `code`, signed
  `priceDelta`, derived required/single-/multiple-choice selection bounds) and
  an ordered Product ↔ Modifier Group assignment with atomic replacement à la
  `expectedVersion` of the product (`V10`).
- Added the `modifier.group.*`, `modifier.option.*` and
  `product.modifiers.manage` permissions (permission catalog 25 → 32).
- Added the Cash Registers foundation: branch-owned Cash Registers with an
  immutable normalized slug `code` (unique per branch), a required `name` and an
  `ACTIVE`/`INACTIVE` status, plus branch-scoped authorization and append-only
  audit (`V11`).
- Added the `cash-register.read`, `cash-register.create` and
  `cash-register.update` permissions (permission catalog 32 → 35).
- Added the Cash Shifts foundation: register-owned Cash Shifts with an
  `OPEN → CLOSED` lifecycle, server-derived actors and timestamps, `BigDecimal`
  (`NUMERIC(19,4)`) declaration amounts (`>= 0`, at most 4 decimals), a
  database-enforced "at most one OPEN shift per register" partial unique index,
  same-organization composite foreign keys, branch-scoped authorization and
  append-only audit for open and close (`V12`).
- Added the `cash-shift.read`, `cash-shift.open` and `cash-shift.close`
  permissions (permission catalog 35 → 38).
- Added the Self Operational Context: `GET /api/v1/auth/me` now returns the
  organization operational context (`organization.currency/locale/timezone`,
  server-derived) and a new self-scoped `GET /api/v1/auth/me/branches`
  (paginated) lists the branches visible to the identity — membership-scoped by
  default, organization-wide with `branch.read`. Visibility only; a deactivated
  branch stays discoverable to its members.
- Read-recovery after branch deactivation: Cash Register and Cash Shift reads
  (and cash-shift close) no longer require the branch to be ACTIVE, so an
  already-open resource stays reachable; writes still require an ACTIVE branch.

## [0.1.0] - 2026-10-10

### Foundation

- Established the modular monolith foundation with Java 21, Spring Boot,
  PostgreSQL, Flyway and the React frontend shell.
- Added Organization and Branch administration.
- Added authentication with short-lived JWT access tokens, rotating refresh
  sessions, logout and session revocation.
- Added Users administration.
- Added database-authoritative Roles, Permissions and RBAC.
- Added Branch Memberships and branch-scoped authorization.
- Added append-only administrative Audit.
- Added secure one-shot provisioning of the first Organization and administrator.
- Hardened production configuration, tenant isolation, concurrency,
  optimistic locking, CORS and database migration paths.
- Established proprietary distribution and private-repository policy.

### Security

- Deny-by-default HTTP security.
- JWT remains identity-only; permissions are resolved from PostgreSQL.
- Cross-organization and cross-branch authorization boundaries.
- Append-only Audit with PostgreSQL enforcement.
- No default credentials.
- No SUPER_ADMIN or HiQNet platform account inside customer POS deployments.

### Known limitations

This release contains the product Foundation only.

The following are not yet implemented:

- Product catalog.
- Cash registers and shifts.
- Sales and payments.
- Tickets.
- Inventory.
- Purchasing.
- Expenses.
- Reports.
- Production administration frontend.
- POS hardware integration.
- HiQNet Control Plane.

HiQNet POS v0.1.0 is a private pre-1.0 technical milestone and is not the
commercially complete POS product.