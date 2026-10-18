# ADR-0019: Recepción de mercancía con actualización atómica de stock

- **Estado:** Aceptada
- **Fecha:** 2026-10-18

## Contexto

ADR-0016 entregó la fundación de órdenes de compra (`V23`) y ADR-0017 las
transiciones de emisión y cancelación. Quedó **diferida** la recepción de
mercancía: confirmar stock real al recibir total o parcialmente una orden emitida,
sin crear balances/movimientos manuales ni exigir la versión del balance.

Restricciones del incremento: base `dev @ e0dfb1db`, versión `0.4.0-SNAPSHOT`,
última migración `V24`; `V1–V24` inmutables; sin frontend; sin devoluciones,
cuentas por pagar ni valoración de inventario; `BigDecimal` exacto;
`organization`/`branch` como límites estrictos de aislamiento; RBAC
deny-by-default con lectura y escritura separadas; OpenAPI como fuente de verdad.

## Decisión

1. **Migración `V25`** (`V25__create_purchasing_purchase_receipts.sql`): crea
   `purchase_receipts` y `purchase_receipt_lines`; amplía `inventory_movements`
   con `purchase_receipt_line_id` y el tipo `PURCHASE_RECEIPT`; amplía
   `purchase_orders.status` a `VARCHAR(24)` con `PARTIALLY_RECEIVED` y `RECEIVED`;
   añade los permisos `purchasing.receipt.read` y `purchasing.receipt.write`
   (catálogo 57 → 59) y las superclaves compuestas que las FKs multisucursal
   requieren.
2. **Identidad idempotente respaldada por PostgreSQL.** `UNIQUE (organization_id,
   purchase_order_id, idempotency_key)`. La clave es obligatoria, de hasta 128
   caracteres. El servicio, **bajo el bloqueo de la cabecera**, busca la recepción
   por clave: si existe y el `payload_fingerprint` (SHA-256 canónico de las líneas
   ordenadas por `id`, cantidades a escala 6) coincide, devuelve el resultado
   original (`200`) **sin efectos nuevos**; si difiere, responde
   `409 PURCHASE_RECEIPT_IDEMPOTENCY_CONFLICT`.
3. **Una transacción, un bloqueo.** `PurchaseReceiptService.post` abre una única
   transacción Spring. Orden de bloqueo: `PurchaseOrder → InventoryItem →
   BranchInventory`. La cabecera se bloquea primero (serializa recepción,
   emisión y cancelación); los artículos se bloquean en `id` ascendente vía
   `InventoryItemUnitGuard` (`SELECT ... FOR UPDATE` + `unit_locked`); los
   balances se crean si faltan (`ensureBalances`) y se bloquean en `id`
   ascendente. La comprobación de idempotencia ocurre tras el bloqueo, de modo que
   dos reintentos simultáneos se serializan.
4. **Atomicidad all-or-nothing.** Se validan todas las líneas antes de escribir:
   no vacío, sin partidas duplicadas, cantidades positivas, partida de la orden,
   y `ya recibido + solicitado <= solicitado_ordenado`
   (`409 PURCHASE_RECEIPT_EXCEEDS_ORDERED`). El artículo debe existir
   (`404`), estar `ACTIVE` (`409 INVENTORY_ITEM_INACTIVE`) y conservar su unidad
   base frente al `unit_snapshot` (`409 PURCHASE_ORDER_UNIT_CHANGED`). Cualquier
   fallo revierte cabecera, líneas, balances, ledger y auditoría.
5. **Ledger append-only.** Cada línea de recepción escribe exactamente un
   movimiento `PURCHASE_RECEIPT` positivo con `resulting_quantity` y
   `purchase_receipt_line_id`, con **índice único parcial** sobre
   `purchase_receipt_line_id` (`type = 'PURCHASE_RECEIPT'`). No se invocan los
   caminos manuales de stock: **no** se generan `ADJUSTMENT` ni se exige la
   versión del balance al cliente. Se respeta la política de unidad
   (`fractionalUnitAllowed`) y el rango `NUMERIC(19,6)`.
6. **Estado de la orden gobernado por el agregado.** `PurchaseOrder.applyReceipt`
   acepta `ORDERED`/`PARTIALLY_RECEIVED` y fija `PARTIALLY_RECEIVED` o `RECEIVED`
   según si todas las partidas alcanzan la cantidad solicitada; conserva la
   emisión y toca `updatedAt` para incrementar `@Version`. El estado nunca se
   acepta del cliente. `expectedVersion` se exige solo en solicitudes nuevas
   (`409 CONCURRENT_MODIFICATION` si está obsoleto); un reintento idéntico
   devuelve el resultado almacenado sin exigir versión.
7. **Cancelación prohibida con recepciones.** `PurchaseOrderService.cancel`
   consulta el número de recepciones **bajo el mismo bloqueo de cabecera** y
   responde `409 PURCHASE_ORDER_HAS_RECEIPTS`; el stock ya ingresado no se
   compensa en este incremento.
8. **Auditoría transaccional.** Cada recepción confirmada registra exactamente un
   evento `purchase_receipt.posted` (entidad `purchase_receipt`) con metadata no
   sensible (`purchaseOrderId`, `purchaseReceiptId`, `branchId`, `lineCount`). Un
   reintento idéntico no audita de nuevo. Si falla la auditoría, todo revierte.
9. **RBAC deny-by-default, lectura separada de escritura.**
   `purchasing.receipt.read` (consultas, con `IgnoringBranchStatus` para que el
   historial siga disponible si la sucursal se desactiva) y
   `purchasing.receipt.write` (recepción, exige `Branch` ACTIVE porque muta
   stock). Se añaden los matchers explícitos en `SecurityConfig`. Ningún rol
   recibe los permisos automáticamente.
10. **Contrato HTTP** bajo `.../purchase-orders/{purchaseOrderId}`:
    `POST /receipts` (`201` nuevo, `200` replay idéntico),
    `GET /receipts`, `GET /receipts/{purchaseReceiptId}` y
    `GET /receipt-status`. OpenAPI se actualiza en el mismo cambio.
11. **Integridad reforzada en PostgreSQL, no solo en Java.** La base impone:
    (a) el movimiento `PURCHASE_RECEIPT` comparte **organización, sucursal y
    artículo** con su línea de recepción, vía la FK compuesta
    `inventory_movements_purchase_receipt_line_fk`
    `(purchase_receipt_line_id, organization_id, branch_id, inventory_item_id)`
    contra la superclave `(id, organization_id, branch_id, inventory_item_id)` de
    `purchase_receipt_lines`; (b) la línea pertenece a la **misma organización y
    sucursal** que su cabecera, vía `purchase_receipt_lines_receipt_branch_fk`;
    (c) `purchase_receipts` y `purchase_receipt_lines` son **inmutables**: triggers
    `BEFORE UPDATE OR DELETE` (mismo mecanismo y `ERRCODE` que el ledger de V17)
    rechazan toda mutación, y el `INSERT` de cabecera y líneas dentro de la misma
    transacción permanece permitido. La anulación de una recepción confirmada sigue
    fuera de alcance.

## Consecuencias

- La recepción es **funcional de punta a punta** (API → PostgreSQL) con
  aislamiento multiempresa, concurrencia serializada, idempotencia estricta y
  auditoría completa.
- La unicidad de la clave, la identidad movimiento↔línea (organización,
  sucursal y artículo) y la inmutabilidad de la recepción las impone PostgreSQL
  (`UNIQUE`, FKs compuestas, índice único parcial y triggers `BEFORE UPDATE OR
  DELETE`), no solo Java.
- Recepción, emisión y cancelación comparten el bloqueo de cabecera: sin
  mutaciones parciales ni `PARTIALLY_RECEIVED`/`RECEIVED` inconsistentes.
- **`V1–V24` intactas**; `V25` es aditiva salvo la ampliación de
  `purchase_orders.status` (`VARCHAR(16) → VARCHAR(24)`) y el `DROP`+`ADD` de los
  `CHECK` de estado y de tipos del ledger.
- Cobertura nueva: recepción completa/parcial, sobrerrecepción atómica,
  idempotencia (replay y conflicto), versión obsoleta, estados no recibibles,
  cancelación con recepciones, RBAC de lectura/escritura y aislamiento por orden,
  más concurrencia (clave idéntica y claves distintas) y constraints en
  PostgreSQL.

## Decisiones diferidas

- Devoluciones de mercancía y movimientos compensatorios (implican un tipo de
  movimiento negativo ligado a la recepción).
- Valoración contable de inventario y costeo (el `unitCost` de la partida no
  alimenta ninguna valoración todavía).
- Cuentas por pagar, facturación y pagos al proveedor.
- `PATCH`/anulación de una recepción ya confirmada: la recepción es inmutable.
