# Changelog

All notable changes to HiQNet POS are documented in this file.

## [0.2.0] - 2026-10-12

### POS Core

- Added an organization-owned operational catalog for branch POS discovery,
  including categories, active products and assigned modifier configuration.
- Added organization-owned modifier groups/options and ordered product-to-group
  assignments.
- Added branch-owned cash registers and register-owned cash shifts, with
  authenticated operational branch context and recovery reads for inactive
  branches/registers.
- Added the POS session UI, product browser and server-persisted cart. Sales are
  created explicitly; the server Sale detail remains the cart and pricing
  authority.

### Sales & Checkout

- Added scoped Sales with retained item/modifier snapshots, exact server pricing,
  optimistic concurrency and the `OPEN → CANCELLED` lifecycle.
- Added atomic checkout to complete a Sale and create immutable CASH/CARD payment
  facts, including split tender and exact cash change calculations.
- Added checkout UI with ephemeral tender input and explicit completion; the
  backend remains authoritative for amounts and checkout state.

### Security & RBAC

- Added database-authoritative permissions for catalog discovery, modifiers,
  cash registers, cash shifts, Sales and checkout, including branch-scoped
  authorization.
- Added self-scoped authenticated operational context and visible-branch
  discovery; branch visibility does not replace authorization on operations.
- Preserved deny-by-default security, tenant isolation and permission resolution
  from PostgreSQL.

### Reliability

- Coordinated Sale mutations and CashShift close with scoped locking and
  optimistic concurrency; checkout and audit changes are transactional.
- Added constraints and same-organization foreign keys for operational and
  financial records.
- Added integrated Chromium E2E assertions against PostgreSQL for persisted
  Sale, Payment and audit data; inactive branch/register reads remain available
  for operational recovery.

### Testing

- CI verifies backend, frontend, Docker Compose and three independent integrated
  POS browser E2E runs against PostgreSQL.
- Added exact decimal, concurrency, authorization, migration and persistence
  coverage for POS Core.

### Scope

This is a private, pre-1.0 POS Core technical milestone, not a commercially
complete POS release. It does not include Inventory, Purchasing, Expenses,
Reports, tickets, actual card processing or hardware integration. Inventory is
not part of this release.

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
