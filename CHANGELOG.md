# Changelog

All notable changes to HiQNet POS are documented in this file.

## [Unreleased]

### POS Core

Development for v0.2.x has started.

- Added the Catalog Foundation: organization-owned Categories and Products
  with an immutable normalized `code`/`sku`, an optional per-organization
  unique `barcode`, a base `unitPrice` (currency inherited from the
  organization) and optional same-organization category assignment (`V9`).
- Added the `category.*` and `product.*` permissions (permission catalog
  19 → 25).

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