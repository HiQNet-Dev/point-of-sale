# Modelo de órdenes de compra — V23

**Estado:** implementado en `V23__create_purchasing_purchase_order_foundation.sql`.

## Ownership y ciclo de vida

`purchase_orders` es un documento propiedad de una Organization con una Branch de
destino **obligatoria** (`branch_id`). Toda consulta de aplicación incluye
`organization_id` y `branch_id`: un UUID de orden no se resuelve globalmente, y una
orden de otra sucursal de la misma organización tampoco es accesible.

En esta migración solo puede persistirse el estado `DRAFT`; `ORDERED` y
`CANCELLED` se contemplan en el `CHECK` porque las transiciones llegan en un
incremento posterior. No existe eliminación física de órdenes: la baja será
`CANCELLED`.

## `purchase_orders`

| Columna | Regla |
| --- | --- |
| `id` | UUID, PK generado por backend. |
| `organization_id` | FK a `organizations`. |
| `branch_id` | Sucursal de destino; FK compuesta con `organization_id`. |
| `supplier_id` | Proveedor; FK compuesta con `organization_id`. |
| `code` | `VARCHAR(64)`, slug en minúsculas, inmutable y único por organización. |
| `status` | `DRAFT` / `ORDERED` / `CANCELLED`. |
| `currency` | `VARCHAR(3)` ISO 4217, copiada de la organización al crear. |
| `supplier_code_snapshot`, `supplier_name_snapshot` | Snapshots del proveedor. |
| `created_by_user_id`, `issued_by_user_id`, `cancelled_by_user_id` | Actores; FK compuesta con `organization_id`. |
| `version` | Control optimista del agregado completo. |
| `created_at`, `updated_at`, `issued_at`, `cancelled_at` | Instantes UTC. |

Integridad: `UNIQUE (organization_id, code)`; `UNIQUE (id, organization_id)`
(superclave para las partidas); CHECK de formato de código y de moneda; y
`purchase_orders_state_consistency`, que exige los actores y timestamps que
corresponden a cada estado (DRAFT sin emisión ni cancelación, ORDERED con
emisión, CANCELLED con cancelación).

Índices: `(organization_id, branch_id, status, created_at DESC, id DESC)` para el
listado; `(organization_id, supplier_id)` para el filtro por proveedor.

## `purchase_order_lines`

| Columna | Regla |
| --- | --- |
| `id` | UUID, PK estable: las recepciones futuras lo referenciarán. |
| `organization_id` | FK compuesta con `purchase_order_id` e `inventory_item_id`. |
| `purchase_order_id` | Orden propietaria. |
| `inventory_item_id` | `InventoryItem` de la misma organización. |
| `line_number` | Asignado por el servidor (`max + 1`), único por orden. |
| `item_code_snapshot`, `item_name_snapshot`, `unit_snapshot` | Snapshots inmutables del artículo. |
| `quantity` | `NUMERIC(19,6)`, estrictamente positivo. |
| `unit_cost` | `NUMERIC(19,4)` opcional, `>= 0`. |

Integridad: `UNIQUE (purchase_order_id, line_number)` y
`UNIQUE (purchase_order_id, inventory_item_id)` — un artículo aparece una sola vez
por orden. `UNIQUE (id, organization_id)` es la superclave para el futuro vínculo
con las líneas de recepción.

## Concurrencia y auditoría

La cabecera es el punto de sincronización del agregado: cada mutación de cabecera
o de partida bloquea la fila scoped de forma pesimista, valida `expectedVersion` y
marca el agregado como modificado, de modo que `version` avanza también cuando el
cambio es una partida. PostgreSQL conserva la última palabra en la unicidad de
código.

Cada creación o cambio real escribe un evento append-only en `audit_events` dentro
de la misma transacción: `purchase_order.created`, `purchase_order.updated`,
`purchase_order.line.added`, `purchase_order.line.updated` y
`purchase_order.line.removed`. La metadata solo lleva código, número de línea,
identificador de artículo, cantidad y nombres de campos modificados.

## Fuera de alcance de V23

V23 no crea recepciones, cantidades recibidas, movimientos de inventario, costeo,
valoración, cuentas por pagar, pagos, impuestos ni devoluciones. Tampoco modifica
`branch_inventory`, `inventory_movements` ni `inventory_items`.
