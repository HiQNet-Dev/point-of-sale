# ADR-0016: Fundación de órdenes de compra para Purchasing

- **Estado:** Aceptada
- **Fecha:** 2026-10-15

## Contexto

`v0.4.x Purchasing` ya tiene proveedores organization-owned (`V22`, ADR-0015). El
siguiente incremento modela la **orden de compra**, el documento que expresa un
compromiso de compra hacia un proveedor y prepara la futura recepción de
mercancía en una sucursal. Antes de recepcionar nada hace falta una cabecera con
partidas que congele el precio pactado y las referencias de catálogo, sin tocar
inventario, sin costeo y sin cuentas por pagar.

Las convenciones vigentes asignan los catálogos administrativos a la
Organization, las operaciones físicas a la Branch, autorizan por permisos
database-authoritative, congelan snapshots en las líneas (como `sale_items`) y
registran auditoría append-only en la misma transacción.

## Decisión

1. `PurchaseOrder` es propiedad de una **Organization** y tiene una **Branch de
   destino obligatoria**. La organización es el scope de autorización; la sucursal
   es el destino logístico y el scope de la recepción futura. Las rutas son
   **branch-nested**, coherentes con `cash-registers`, `inventory-items` y `sales`.
2. `code` es un slug manual, inmutable, normalizado en minúsculas y **único por
   Organization**. No se introduce secuencia generada por servidor: todos los
   códigos del repositorio son slugs manuales y PostgreSQL sigue siendo la
   autoridad final de unicidad.
3. Cada partida referencia un **`InventoryItem`** (no un `Product`, que es
   vendible) y congela snapshots inmutables de `code`, `name` y `baseUnit`. Los
   cambios de catálogo nunca reescriben el historial de la orden.
4. `currency` es un **snapshot obligatorio de `organization.currency`** fijado por
   el backend; el cliente no puede fijarla ni modificarla. `unit_cost` es
   opcional, `NUMERIC(19,4)` y admite cero. **No** se almacenan totales, ni
   valoración de inventario, ni impuestos, ni cuentas por pagar.
5. Los estados del modelo son `DRAFT`, `ORDERED` y `CANCELLED`. Este incremento
   solo permite **crear y editar borradores**: `issue` y `cancel` se implementan
   en el PR 2.2. El estado **nunca** se acepta desde el cliente; la ampliación a
   `PARTIALLY_RECEIVED`/`RECEIVED` llega con las recepciones.
6. El **agregado se sincroniza en la fila de cabecera**: toda mutación de cabecera
   o de partida toma primero un bloqueo pesimista scoped
   (`id + organization_id + branch_id`), valida `expectedVersion` y deja el
   agregado sucio para que `@Version` incremente la versión al hacer flush. Las
   partidas viven en su propia tabla, por lo que insertar, actualizar o eliminar
   una línea **no** incrementa la versión por sí solo: el servicio toca la
   cabecera de forma explícita.
7. Las partidas tienen **identidad estable** (CRUD incremental, no reemplazo
   total) porque las recepciones futuras las referenciarán. Solo son editables en
   `DRAFT`; en `DRAFT` la eliminación es **física** (`POST .../lines/{id}/remove`),
   porque nada las referencia todavía y la baja queda auditada.
8. Cuatro permisos nuevos: `purchasing.order.read`, `purchasing.order.write`,
   `purchasing.order.issue` y `purchasing.order.cancel`. La emisión y la
   cancelación se separan de la edición porque son transiciones privilegiadas.
   Sin auto-grants a roles existentes.

## Consecuencias

- El historial de una orden no se reescribe ante renombrados de proveedor o de
  artículo: las líneas y la cabecera conservan sus snapshots.
- El orden de adquisición de bloqueos queda fijado en
  `PurchaseOrder → PurchaseOrderLine → (futuro) BranchInventory`, igual que la
  convención de Sales, evitando deadlocks cuando la recepción mute existencias.
- La versión de la cabecera es el contrato de concurrencia visible al cliente: dos
  escrituras concurrentes sobre el mismo agregado se serializan y la segunda
  recibe `409 CONCURRENT_MODIFICATION`; no hay actualizaciones perdidas.
- Al no modelarse `ORDERED` ejecutable todavía, ninguna orden puede emitirse ni
  cancelarse por API en este incremento.
- Este incremento **no** crea movimientos de inventario ni altera `branch_inventory`.

## Decisiones diferidas

- Transiciones `issue` y `cancel` (PR 2.2), con sus actores y timestamps.
- Recepción de mercancía: cantidades recibidas, recepciones parciales,
  idempotencia, estados `PARTIALLY_RECEIVED`/`RECEIVED` y el tipo de movimiento
  `PURCHASE_RECEIPT`.
- Volver inmutable `InventoryItem.baseUnit` cuando existan dependencias
  (ADR-0009 §4): trabajo independiente obligatorio antes de las recepciones.
- Numeración legible generada por servidor, si el negocio la exige.
- Costeo, valoración de inventario, cuentas por pagar, pagos, impuestos y
  devoluciones.
