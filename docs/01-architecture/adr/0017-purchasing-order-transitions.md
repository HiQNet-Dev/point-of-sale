# ADR-0017: Transiciones de órdenes de compra (emisión y cancelación)

- **Estado:** Aceptada
- **Fecha:** 2026-10-15

## Contexto

ADR-0016 entregó la **fundación** de órdenes de compra (`V23`): el agregado
`PurchaseOrder` con partidas, snapshots de proveedor y artículo, bloqueo pesimista
scoped de la cabecera, `expectedVersion` y auditoría transaccional. En esa
fundación todas las órdenes quedaban en `DRAFT`: la emisión y la cancelación
quedaron **diferidas** (ADR-0016 §Decisiones diferidas).

El siguiente paso es ejecutar la máquina de estados
`DRAFT → ORDERED → (recepción)` con `CANCELLED` alcanzable desde `DRAFT` u
`ORDERED`, sin implementar todavía la recepción de mercancía.

## Decisión

1. **Transiciones gobernadas por el agregado.** `PurchaseOrder.issue(actor, when)`
   solo es válida desde `DRAFT` (`409 PURCHASE_ORDER_NOT_DRAFT` en otro caso);
   `PurchaseOrder.cancel(actor, when)` es válida desde `DRAFT` u `ORDERED`
   (`409 PURCHASE_ORDER_ALREADY_CANCELLED` si ya estaba cancelada). La cancelación
   es **irreversible** y, si la orden ya estaba emitida, **conserva** `issued_at` e
   `issued_by_user_id`; en un borrador esos campos permanecen `NULL`. Lo garantiza
   el `CHECK purchase_orders_state_consistency` de `V23`, que no requiere cambios.
2. **Actores y fechas siempre del servidor.** `issued_at`/`issued_by_user_id` y
   `cancelled_at`/`cancelled_by_user_id` se fijan desde el usuario autenticado y el
   `Clock`; el estado nunca se acepta por `PATCH` ni desde el cliente.
3. **Se reutiliza el contrato de concurrencia existente.** Ambas transiciones
   toman el mismo bloqueo pesimista scoped de la cabecera y validan
   `expectedVersion`; ninguna crea un mecanismo de concurrencia alternativo. La
   emisión amplía el orden de bloqueo a `PurchaseOrder → Supplier → InventoryItem`
   (los artículos en `id` ascendente); la cancelación sigue en
   `PurchaseOrder → PurchaseOrderLine` y las mutaciones de partida añaden el
   bloqueo de `InventoryItem` (el alta de partida bloquea el artículo vía el
   guard de unidad). **No** se bloquea
   `BranchInventory` en este incremento.
4. **Validación en la emisión.** Antes de emitir se exige al menos una partida
   (`409 PURCHASE_ORDER_EMPTY`), proveedor `ACTIVE` (`409 SUPPLIER_INACTIVE`),
   artículos `ACTIVE` (`409 INVENTORY_ITEM_INACTIVE`) y que la **unidad base
   actual** de cada artículo coincida con el `unit_snapshot` capturado
   (`409 PURCHASE_ORDER_UNIT_CHANGED`). La captura de la partida congela el
   snapshot; la emisión es el punto donde se exige que siga siendo válido. La
   validación es una **transición atómica** frente a los cambios administrativos
   concurrentes: la emisión bloquea el proveedor y después los artículos
   (`SELECT ... FOR NO KEY UPDATE`) tomando bloqueos de fila que conflictúan con
   los `FOR UPDATE` de `SupplierService.update` e `InventoryItemService.update`. Así, o la
   desactivación gana el bloqueo y la emisión observa `INACTIVE` y falla, o la
   emisión gana y confirma antes de que la desactivación prosiga; nunca emite
   sobre un estado obsoleto del Persistence Context (el proveedor se carga de
   forma `EAGER` con la orden; el proveedor y los artículos se releen como
   proyección escalar bajo el bloqueo).
5. **Emitir es un registro interno.** `ORDERED` no implica envío de correos ni
   documentos al proveedor, y **ni emitir ni cancelar** modifican existencias: no
   hay escrituras en `branch_inventory` ni en `inventory_movements`.
6. **Auditoría transaccional.** Cada transición exitosa registra exactamente un
   evento en la misma transacción: `purchase_order.issued` y
   `purchase_order.cancelled` (entidad `purchase_order`), con metadata no sensible
   (`toStatus`, `fromStatus` para la cancelación, `lineCount` para la emisión). Si
   falla la auditoría, la mutación revierte.
7. **Permisos ya existentes.** `purchasing.order.issue` y `purchasing.order.cancel`
   ya estaban en el catálogo (`V23`, 57 permisos) y **no** se conceden
   automáticamente a ningún rol. Se añaden los matchers explícitos
   (`POST .../issue`, `POST .../cancel`) a `SecurityConfig` para que no recaigan en
   el `denyAll`.

## Consecuencias

- El ciclo de vida queda ejecutable por API con las mismas garantías de
  concurrencia, aislamiento multiempresa y auditoría de la fundación.
- Una orden emitida es **inmutable**: `PATCH` de cabecera y mutaciones de partida
  responden `409 PURCHASE_ORDER_NOT_DRAFT`.
- La emisión queda **serializada** con la desactivación del proveedor y de los
  artículos: se cierra la carrera TOCTOU de emisión que ADR-0018 §6 dejaba como
  residual, con el mismo bloqueo pesimista de fila y el orden
  `PurchaseOrder → Supplier → InventoryItem`.
- Las carreras (dos emisiones, dos cancelaciones, emisión vs cancelación, emisión
  vs mutación de partidas) se resuelven en la fila de cabecera: una gana y las
  demás reciben `409`, sin mutaciones parciales ni eventos de auditoría extra.
- **Sin migración nueva**: `V23` ya contiene el `CHECK` de estados y los permisos;
  este incremento no altera V1–V23.
- **Sin endpoints ni permisos nuevos**: la emisión y la cancelación ya existían en
  el contrato (`POST .../issue`, `POST .../cancel`) y en el catálogo de permisos;
  no se añaden rutas, DTOs ni permisos.
- El guard de inmutabilidad de `InventoryItem.baseUnit` **ya está integrado**
  (PR #59: `InventoryItemUnitGuard`); **no** es una dependencia pendiente. Este
  incremento solo añade su detección en la emisión bajo bloqueo. La **recepción de
  mercancía** sigue pendiente y queda fuera de alcance.

## Decisiones diferidas

- Recepción de mercancía: `PARTIALLY_RECEIVED`/`RECEIVED`, cantidades recibidas,
  idempotencia y el movimiento `PURCHASE_RECEIPT`.
- Guard global de Inventory y costeo/valoración; cuentas por pagar y pagos.
