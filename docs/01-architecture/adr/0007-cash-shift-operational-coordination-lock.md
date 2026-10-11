# ADR-0007: CashShift row as operational coordination lock

- **Status:** Accepted
- **Date:** 2026-10-10

## Context

An OPEN Sale must only be created for an OPEN CashShift, and a CashShift must not close while it has any OPEN Sale. A plain check in two separate transactions permits an invalid interleaving: creation sees OPEN, close sees no sales, close commits, then creation inserts an OPEN Sale.

## Decision

`SaleService.create` and `CashShiftService.close` acquire `PESSIMISTIC_WRITE` on the same scoped `cash_shifts` row before evaluating their cross-row invariant. The close path asks `OpenSalesForCashShiftPort`, defined at the cashshift application boundary and implemented by Sales, so CashShift does not depend on a concrete Sales repository.

Sales retain `Sale.version` optimistic concurrency for their normal add/update/remove/cancel aggregate mutations. The pessimistic lock is limited to create-versus-close coordination and is not a general replacement for optimistic concurrency.

## Consequences

The lock serializes only requests competing for one operational shift, preventing CLOSED shift plus OPEN sale state. It can briefly block a concurrent close/create on that shift, which is intentional. Payment, completion, inventory and cash reconciliation remain outside this decision.
