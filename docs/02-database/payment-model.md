# Payments and atomic checkout

`V14__create_payments_and_checkout.sql` extends the Sale lifecycle to `OPEN → CANCELLED` or `OPEN → COMPLETED` and introduces immutable `payments` records.

## Atomic boundary

A payment has no independent write API and no draft, pending, failed, partial-payment, refund, gateway or card-terminal state. `POST .../sales/{saleId}/checkout` validates the complete tender instruction set, inserts all Payment rows, completes the Sale and records one `sale.completed` audit event in one transaction. Any failure rolls all of it back.

Checkout locks the scoped `CashShift` row first (the same coordination row used by shift close), then claims `Sale.version` optimistically before payment child rows are inserted. A CLOSED shift can therefore never retain an OPEN sale. Completed and cancelled sales no longer block shift close.

## Payment facts

A Payment belongs to the exact Organization, Branch, CashRegister, CashShift and Sale through a composite FK. The creating user is same-organization through a composite FK. Records are immutable and ordered from the checkout request (`position`, unique per sale).

Only `CASH` and `CARD` exist. `amount` is the amount applied to the Sale; it is positive and totals must match `Sale.totalAmount` exactly using `BigDecimal` / `NUMERIC(19,4)`.

- CASH requires `tenderedAmount >= amount`; the server derives `changeAmount = tenderedAmount - amount`. At most one cash entry is allowed.
- CARD has no tendered amount and zero change.
- At most ten payments may be supplied. Multiple CARD entries support a split tender.
- A Sale needs at least one ACTIVE item. A zero-total Sale with active items completes with an empty payment array; a positive total requires exact payment coverage.

Checkout never reprices catalog data: SaleItem snapshots and `Sale.totalAmount` remain authoritative. Payments are readable in position order through the separate `payment.read` endpoint so Sale detail never implicitly grants payment visibility.

There is no idempotency key yet because checkout has no external processor side effect. If a client loses the response after commit and retries, the retry returns `SALE_ALREADY_COMPLETED`; the committed Payment set and audit event are not duplicated.

Future CashMovement reconciliation can use CASH payment amounts as a source, but no cash ledger, expected-cash calculation, inventory change, ticket or receipt is created by this slice.
