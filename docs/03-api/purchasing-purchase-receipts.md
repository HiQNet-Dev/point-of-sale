# API de recepción de mercancía de Purchasing

**Estado:** implementada en `v0.4.0` (V25).

## Alcance

Una **recepción** confirma stock real de una orden de compra **emitida**
(`ORDERED` o `PARTIALLY_RECEIVED`) en una sola transacción: crea el balance de
sucursal si falta, escribe un movimiento `PURCHASE_RECEIPT` append-only por línea,
persiste la cabecera y las líneas de recepción, actualiza el estado de la orden
(`PARTIALLY_RECEIVED`/`RECEIVED`) y audita. Es **idempotente** por clave y
**atómica**: todo confirma o revierte junto.

Fuera de alcance de este incremento: devoluciones, valoración contable de
inventario, cuentas por pagar y anulación de una recepción confirmada.

## Rutas

Todas las rutas cuelgan de la orden y están anidadas en organización y sucursal:

```text
POST /organizations/{organizationId}/branches/{branchId}/purchase-orders/{purchaseOrderId}/receipts
GET  /organizations/{organizationId}/branches/{branchId}/purchase-orders/{purchaseOrderId}/receipts
GET  /organizations/{organizationId}/branches/{branchId}/purchase-orders/{purchaseOrderId}/receipts/{purchaseReceiptId}
GET  /organizations/{organizationId}/branches/{branchId}/purchase-orders/{purchaseOrderId}/receipt-status
```

Una orden o recepción de otra sucursal de la misma organización responde `404`:
la sucursal forma parte del ámbito del recurso.

## Permisos

- `purchasing.receipt.read` (branch-scoped, con `IgnoringBranchStatus`): consultas.
- `purchasing.receipt.write` (branch-scoped, exige `Branch` **ACTIVE**): registrar
  una recepción, porque muta stock.

## Registrar una recepción

`POST .../purchase-orders/{purchaseOrderId}/receipts`

```json
{
  "expectedVersion": 2,
  "idempotencyKey": "receipt-2026-10-18-0001",
  "lines": [
    { "purchaseOrderLineId": "…", "quantity": "4.000000" }
  ]
}
```

- `expectedVersion`: versión de la orden leída por el cliente (`@Version`).
- `idempotencyKey`: `1..128` caracteres; **única por organización y orden**.
- `quantity`: string decimal exacto (`NUMERIC(19,6)`), estrictamente positivo, en
  la unidad base congelada en la partida.

Respuesta `201` (nueva) o `200` (reintento idéntico):

```json
{
  "purchaseReceipt": {
    "id": "…", "organizationId": "…", "branchId": "…", "purchaseOrderId": "…",
    "idempotencyKey": "receipt-2026-10-18-0001", "receivedByUserId": "…",
    "receivedAt": "…", "createdAt": "…"
  },
  "lines": [
    { "id": "…", "purchaseOrderLineId": "…", "inventoryItemId": "…", "quantityReceived": "4.000000" }
  ],
  "purchaseOrder": { "…": "estado y versión vigentes tras la recepción" }
}
```

### Idempotencia

- Reintento con la **misma clave y el mismo contenido** (huella SHA-256 canónica
  de las líneas ordenadas por `id`): `200` con el resultado original, **sin**
  efectos nuevos ni auditoría duplicada. No exige `expectedVersion`.
- Misma clave con **contenido distinto**: `409
  PURCHASE_RECEIPT_IDEMPOTENCY_CONFLICT`.
- La unicidad la respalda PostgreSQL (`UNIQUE (organization_id,
  purchase_order_id, idempotency_key)`).

### Validaciones (todo o nada)

1. La orden existe en el ámbito (`404 PURCHASE_ORDER_NOT_FOUND`).
2. La orden es recibible: `ORDERED`/`PARTIALLY_RECEIVED` (`409
   PURCHASE_ORDER_NOT_ISSUED` en `DRAFT`, `409 PURCHASE_ORDER_ALREADY_RECEIVED`,
   `409 PURCHASE_ORDER_ALREADY_CANCELLED`).
3. `expectedVersion` coincide (`409 CONCURRENT_MODIFICATION`).
4. La sucursal está `ACTIVE` (`409 BRANCH_INACTIVE`).
5. Al menos una línea, sin partidas duplicadas, cantidades positivas y partidas de
   la orden (`404 PURCHASE_ORDER_LINE_NOT_FOUND`).
6. `ya recibido + solicitado ≤ solicitado` (`409
   PURCHASE_RECEIPT_EXCEEDS_ORDERED`).
7. Artículo `ACTIVE` (`409 INVENTORY_ITEM_INACTIVE`) y unidad base sin cambios
   frente al `unit_snapshot` (`409 PURCHASE_ORDER_UNIT_CHANGED`); la cantidad
   resultante respeta la política de unidad y `NUMERIC(19,6)`.

## Consultar recepciones

- `GET .../receipts`: lista cronológica de recepciones confirmadas.
- `GET .../receipts/{purchaseReceiptId}`: detalle con líneas y estado vigente de
  la orden.
- `GET .../receipt-status`: cantidades `quantityOrdered`, `quantityReceived` y
  `quantityPending` por partida, con el estado y la `version` de la orden.

## Cancelación

Una orden con recepciones confirmadas **no** puede cancelarse: `POST
.../cancel` responde `409 PURCHASE_ORDER_HAS_RECEIPTS`, bajo el mismo bloqueo de
cabecera que la recepción. El stock ya ingresado no se compensa en este
incremento.

## Concurrencia

La recepción toma primero el bloqueo pesimista de la cabecera, luego los artículos
(`id` ascendente, vía el guard de unidad) y luego los balances. La comprobación de
idempotencia ocurre bajo el bloqueo de la cabecera: dos reintentos simultáneos con
la misma clave se serializan y **solo uno** registra stock y auditoría; dos claves
distintas con la misma `expectedVersion` se serializan y la segunda recibe `409
CONCURRENT_MODIFICATION`, sin doble aplicación de stock.

## Errores relevantes

| Estado | Código | Causa |
| --- | --- | --- |
| `400` | `VALIDATION_ERROR` | Cuerpo inválido, cantidad no positiva o clave ausente/larga. |
| `400` | `INVALID_INVENTORY_QUANTITY` | Cantidad no es un decimal exacto válido. |
| `403` | `FORBIDDEN` | Falta `purchasing.receipt.read`/`write`. |
| `404` | `PURCHASE_ORDER_NOT_FOUND` | Orden inexistente o de otra sucursal. |
| `404` | `PURCHASE_ORDER_LINE_NOT_FOUND` | Partida ajena a la orden. |
| `404` | `PURCHASE_RECEIPT_NOT_FOUND` | Recepción inexistente o de otra orden. |
| `409` | `PURCHASE_ORDER_NOT_ISSUED` / `PURCHASE_ORDER_ALREADY_RECEIVED` | Orden en `DRAFT` o ya completa. |
| `409` | `PURCHASE_RECEIPT_EXCEEDS_ORDERED` | La recepción superaría lo solicitado. |
| `409` | `PURCHASE_RECEIPT_IDEMPOTENCY_CONFLICT` | Misma clave con contenido distinto. |
| `409` | `PURCHASE_ORDER_HAS_RECEIPTS` | Cancelar una orden con recepciones. |
| `409` | `CONCURRENT_MODIFICATION` / `INVENTORY_ITEM_INACTIVE` / `PURCHASE_ORDER_UNIT_CHANGED` / `BRANCH_INACTIVE` | Estado concurrente o inválido. |
