# ADR-0008: Atomic checkout and immutable payments

- **Status:** Accepted
- **Date:** 2026-10-10

## Context

An OPEN Sale can be cancelled or changed until it is financially finalized. Persisting payments independently would create draft/partial state and could leave payment rows attached to an OPEN Sale after a failure.

## Decision

Checkout is one transaction: validate complete tender instructions, persist immutable Payment facts, transition the Sale to `COMPLETED`, and record one `sale.completed` audit event. `Sale.version` remains the aggregate concurrency boundary. Checkout first takes the existing scoped `CashShift` coordination lock, then claims Sale version before inserting payment children.

`CASH` and `CARD` are local accounting facts only. No gateway, terminal data, idempotency key, partial-payment lifecycle, refund, cash movement or receipt is modeled. Repeat checkout after a committed response-loss conflict returns `SALE_ALREADY_COMPLETED` and cannot create another payment set.

## Consequences

The system preserves exact payment coverage and no partial checkout persistence. External authorization/capture and refunds require a later, explicitly designed payment lifecycle and integration boundary.
