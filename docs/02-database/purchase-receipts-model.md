# Modelo de recepciones de compra (`V25`)

Una **recepción** es el hecho inmutable que confirma stock real de una orden de
compra **emitida**. Pertenece a la combinación Organización + Sucursal + Orden y
contiene una o más líneas con cantidades positivas en la unidad base congelada en
la partida. Cada línea origina exactamente un movimiento `PURCHASE_RECEIPT` en el
ledger. Toda la operación es atómica e idempotente. `V1–V24` son inmutables.

## `purchase_receipts` (cabecera)

| Columna | Regla |
| --- | --- |
| `id` | UUID, PK generado por backend. |
| `organization_id` | FK a `organizations`. |
| `branch_id` | Sucursal de destino; FK compuesta `(branch_id, organization_id)`. |
| `purchase_order_id` | Orden recibida; FK compuesta `(purchase_order_id, organization_id, branch_id)`. |
| `idempotency_key` | `VARCHAR(128)`, no vacío; identidad de la recepción. |
| `payload_fingerprint` | `VARCHAR(64)`, SHA-256 canónico de las líneas. |
| `received_by_user_id` | Actor; FK compuesta `(received_by_user_id, organization_id)`. |
| `received_at`, `created_at` | Instantes UTC. |

Integridad: **`UNIQUE (organization_id, purchase_order_id, idempotency_key)`** —
respalda la idempotencia en PostgreSQL, no solo en Java. Superclaves compuestas
`(id, organization_id, branch_id)` y `(id, organization_id, purchase_order_id)`
para las FKs de las líneas y del ledger. La cabecera es **inmutable** en
PostgreSQL: el trigger `purchase_receipts_immutable` (`BEFORE UPDATE OR DELETE`)
rechaza toda mutación; el `INSERT` permanece permitido.

## `purchase_receipt_lines` (líneas)

| Columna | Regla |
| --- | --- |
| `id` | UUID, PK. |
| `organization_id`, `branch_id` | Alcance de la recepción. |
| `purchase_receipt_id` | Cabecera; FK compuesta `(purchase_receipt_id, organization_id, purchase_order_id)`. |
| `purchase_order_id` | Orden recibida. |
| `purchase_order_line_id` | Partida original; FK compuesta `(purchase_order_line_id, organization_id, purchase_order_id)`. |
| `inventory_item_id` | Artículo; FK compuesta `(inventory_item_id, organization_id)`. |
| `quantity_received` | `NUMERIC(19,6) > 0`, escala exacta. |
| `created_at` | Instante UTC. |

Integridad: `UNIQUE (purchase_receipt_id, purchase_order_line_id)` — una recepción
no recibe dos veces la misma partida. `purchase_receipt_lines_receipt_branch_fk`
obliga a que la línea pertenezca a la **misma organización y sucursal** que su
cabecera. La superclave `(id, organization_id, branch_id, inventory_item_id)`
permite que el ledger fije el artículo de cada movimiento. Las líneas son
**inmutables**: el trigger `purchase_receipt_lines_immutable` (`BEFORE UPDATE OR
DELETE`) rechaza toda mutación; el `INSERT` permanece permitido.

## Ledger `inventory_movements` (extensión)

- Nuevo tipo `PURCHASE_RECEIPT`, **siempre positivo**, `sale_id` nulo y
  `purchase_receipt_line_id` no nulo (lo impone `inventory_movements_type_values`).
- **Índice único parcial** `inventory_movements_one_receipt_per_line` sobre
  `purchase_receipt_line_id` (`WHERE type = 'PURCHASE_RECEIPT'`): un movimiento por
  línea de recepción.
- **FK de identidad** `inventory_movements_purchase_receipt_line_fk`
  `(purchase_receipt_line_id, organization_id, branch_id, inventory_item_id)` →
  `purchase_receipt_lines (id, organization_id, branch_id, inventory_item_id)`:
  PostgreSQL garantiza que el movimiento comparte organización, sucursal y
  artículo con su línea, no solo la aplicación.

## `purchase_orders` (extensión de estado)

- `status` pasa a `VARCHAR(24)`; el `CHECK` admite `PARTIALLY_RECEIVED` y
  `RECEIVED` además de `DRAFT`/`ORDERED`/`CANCELLED`.
- `purchase_orders_state_consistency` exige que `ORDERED`, `PARTIALLY_RECEIVED` y
  `RECEIVED` conserven la emisión (`issued_at`/`issued_by_user_id` no nulos, sin
  cancelación).

## Escrituras y bloqueos

La recepción abre una única transacción con orden de bloqueo
`PurchaseOrder → InventoryItem → BranchInventory`, crea los balances faltantes
(`ensureBalances`) y los bloquea en `id` ascendente. Escribe las líneas antes que
los movimientos (la FK lo exige) y actualiza el estado de la orden bajo el mismo
bloqueo de cabecera. Un fallo revierte cabecera, líneas, balances, ledger y
auditoría.

## Garantías de aplicación vs garantías de PostgreSQL

| Invariante | Garantía |
| --- | --- |
| Organización, sucursal y artículo del movimiento = los de su línea | PostgreSQL (FK compuesta) |
| Línea y cabecera comparten organización y sucursal | PostgreSQL (FK compuesta) |
| Un movimiento por línea; clave idempotente única por organización y orden | PostgreSQL (índice único parcial / `UNIQUE`) |
| Cabecera y líneas inmutables tras el `INSERT` | PostgreSQL (triggers `BEFORE UPDATE OR DELETE`) |
| No recibir más de lo pedido, artículo `ACTIVE`, unidad congelada, estado de la orden | Aplicación (servicio, bajo bloqueo) |
| Orden de bloqueo `PurchaseOrder → InventoryItem → BranchInventory` | Aplicación sobre locks de PostgreSQL |
| Idempotencia lógica (fingerprint, replay `200` vs `409`) | Aplicación sobre la unicidad de PostgreSQL |

## Fuera de alcance

Devoluciones y movimientos compensatorios, valoración/costeo de inventario,
cuentas por pagar y anulación de una recepción confirmada (prohibida además por
los triggers de inmutabilidad).
