# Contrato de paridad de compras Java → Rust — incremento 0c

**Fecha:** 2026-10-28. **Estado:** auditoría de origen y propuesta de mapping;
**NO ADOPTADO como resolución de diseño**. [ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md)
permanece **PROPOSED / PROPUESTO SIN APROBACIÓN**. Este contrato no lo acepta,
no modifica F1–10/F1–12/F4–21 y no desbloquea full completion 0b.

## 1. Alcance, autoridad y método

- Rama inspeccionada: `feat/backend/rust-physical-resolution`;
  HEAD inicial `6a15ee6a21ef80c17ab7276b337946cbcc4a4d37`, checkout limpio.
- Baseline de origen: `ebf7a30afc342a31d837a85772c5e85d0bc38efb`.
  El diff contra ese SHA no mostró cambios en `backend/src/main`, ni en las
  pruebas de `purchasing` e `inventory/stock` inspeccionadas. No se audita una BD
  de cliente ni se afirma que existan registros productivos con estos valores.
- Se contrastaron Flyway V23–V25, entidades, servicios, DTOs/controllers y pruebas
  de órdenes, recepciones y ledger; V17 aporta el ledger previo. V24 no añade
  columnas a las cuatro tablas, pero fija el significado histórico de unidades.
- Único entregable/escritura: este Markdown. Sin DDL, cambios Java/Rust,
  modificaciones al ADR, contratos HTTP, importaciones ni cutover.
- Las referencias de Drive son mutables. Se consultaron F1–10, F1–12, F4–13 y
  F4–15 por TXT; se consignan referencias y conclusiones mínimas de paridad, no
  copias del expediente confidencial, tablas completas de diseño ni diagramas.
- Autoridad: Java/Flyway describe el hecho AS-IS; F1–12 exige su preservación;
  F1–10 fija el destino físico; F4–15/F4–13, ratificados por F4–21, fijan estados,
  scope y costeo. Una contradicción se mantiene abierta: no se elige una fuente
  para borrar la otra. OpenAPI sigue siendo la autoridad HTTP del repositorio;
  aquí se inspeccionaron anotaciones/DTOs, no `/v3/api-docs` en ejecución.

**Resultado:** hay **47 columnas de origen: 17 + 12 + 9 + 9**, todas enumeradas en
§3. Que exista una ficha de tabla destino no demuestra que todos sus hechos
quepan. El bloqueo ya descrito en `rust-physical-audit.md` §4–6 persiste.

### Leyenda de certeza del destino

| Marca   | Significado                                                                                                                              |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **C**   | El campo destino está enumerado en la sección citada de F1–10. Confirma existencia documental, no implementación ni import aprobado.     |
| **C/T** | Campo enumerado, pero requiere transformación/interpretación explícita de paridad. La regla propuesta se indica; no se da por ejecutada. |
| **C/!** | Campo enumerado con restricción incompatible o información insuficiente en origen. Import bloqueado hasta resolución.                    |
| **P**   | Nombre/ubicación propuestos para conservar un hecho sin destino explícito suficiente en esa ficha. No son campos físicos aprobados.      |

Los nombres de tipos Rust en este documento son responsabilidades de dominio,
no structs/módulos implementados. Una fila **P** no autoriza añadir una columna,
una tabla auxiliar, JSON libre ni eliminar una FK. Las referencias **S** y **N**
se resuelven con rutas y secciones exactas en §9.

## 2. Hechos de origen que gobiernan el mapping

1. **Moneda y proveedor históricos.** La moneda se copia de la organización al
   crear la orden; el cliente no la fija ni modifica. `supplier_id` apunta al
   proveedor y los snapshots guardan su código/nombre capturados. Cambiar a otro
   proveedor en DRAFT recaptura ambos; reenviar el mismo proveedor es no-op del
   servicio. No reconstruirlos desde el proveedor actual ni copiar la moneda
   actual de la organización sobre la histórica. [S23, S-PO, S-OS]
2. **Partidas.** ID, artículo, número de línea y snapshots de código/nombre/unidad
   se conservan; cambiar artículo exige quitar/agregar una partida en DRAFT.
   Cantidad y costo solo se editan en DRAFT. El borrado físico de esas partidas
   editables no equivale a borrar recepciones confirmadas. [S-PL, S-OS]
3. **Costo pactado opcional, no valoración.** `unit_cost` es nullable, no negativo
   cuando existe; cero es un valor distinto de desconocido. Ninguna validación de
   emisión/recepción exige completarlo. El fixture del test de transición omite
   costo y alcanza ORDERED; el de recepción omite costo y alcanza RECEIVED con
   stock ingresado. Es evidencia fuente, no ejecución en 0c. [S23, S-DEC, T02, T03]
4. **Recepción de cantidades, no factura.** El request solo aporta versión, clave
   y pares partida/cantidad. Java no guarda costo efectivo, total, impuestos,
   fletes, descuentos, rechazos, valoración ni pago propios de recepción.
   `quantity_received` es cantidad ingresada a stock, en la unidad snapshot de la
   partida; no cantidad entregada antes de rechazos. [S25, S-RL, S-API-R]
5. **Unidades bloqueadas.** V24 backfillea `unit_locked` por dependencias,
   incluidas partidas de compra; el marcador persiste aunque desaparezca una
   dependencia DRAFT. Alta y recepción usan la guarda del artículo; emisión y
   recepción rechazan divergencia entre unidad snapshot y base actual. No
   convertir GRAM a otra unidad ni reemplazar snapshots durante el import.
   [S24, S-OS `addLine`/`lockIssuableItems`, S-RS `validateItems`]
6. **Actores y tiempos no son intercambiables.** Se conservan creador, emisor,
   cancelador y receptor, incluidos NULL de emisión/cancelación. Tiempos de
   negocio usan `Clock`; callbacks JPA asignan creación/actualización mediante
   `Instant.now()` y JDBC asigna `occurred_at` con `now()`. No imponer igualdad
   entre `received_at`, `created_at`, `updated_at` y fecha del movimiento.
   [S-PO, S-PL, S-RH, S-RL, S-JDBC]

## 3. Mapping exhaustivo campo por campo

Convenciones: **NN** = NOT NULL de origen, **N** = nullable. UUID se conserva,
no se regenera; destino local usa UUID canónico. Instantes pasan de TIMESTAMPTZ a
UTC ISO-8601 sin pérdida de precisión histórica. Decimales pasan a contratos
exactos/cadenas canónicas, nunca `REAL`, `f32/f64` ni JSON Number. La
representación PostgreSQL nueva no autoriza redondeo implícito. [N10 §2, N12 §4]

### 3.1 `purchase_orders` → `purchase_order` — 17/17

Origen: S23 L12–60; evolución de estado en S25 L132–155; S-PO campos y lifecycle.
Destino físico de referencia: **N10 §16.16**.

| Columna origen / tipo                    | Destino Rust/local                                                                               | Certeza y regla de preservación                                                                                                                                                                             |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id` UUID NN                             | `purchase_order.id`                                                                              | **C**. Mismo UUID del agregado.                                                                                                                                                                             |
| `organization_id` UUID NN                | `purchase_order.organization_id`                                                                 | **C**. Organization propietaria; no deducir autorización del UUID.                                                                                                                                          |
| `branch_id` UUID NN                      | `purchase_order.branch_id`                                                                       | **C**. Sucursal de destino inmutable; validar pareja organización/sucursal.                                                                                                                                 |
| `supplier_id` UUID NN                    | `purchase_order.supplier_id`                                                                     | **C**. Misma identidad; FK con organización hacia `supplier` (N10 §17.15), independiente de snapshots.                                                                                                      |
| `code` VARCHAR(64) NN                    | Propuesto `purchase_order.code`                                                                  | **P**. Preservar slug e identidad comercial; unicidad `(organization_id, code)` del origen. No sustituirlo por UUID ni regenerar folio.                                                                     |
| `status` VARCHAR(24) NN                  | `purchase_order.status`                                                                          | **C/T**. Conservar los cinco estados Java; no convertir parcial en completo/cancelado. Extensión CLOSED_PARTIAL se distingue en §5.                                                                         |
| `currency` VARCHAR(3) NN                 | Propuesto `purchase_order.currency`; alimenta `purchase_order_line.currency` enumerado en §16.40 | **P** en cabecera; **C/T** en línea. Copiar moneda histórica de la orden, incluso sin líneas; no basta guardarla solo en líneas.                                                                            |
| `supplier_code_snapshot` VARCHAR(64) NN  | Propuesto `purchase_order.supplier_code_snapshot`                                                | **P**. Texto histórico exacto, no código del supplier vigente.                                                                                                                                              |
| `supplier_name_snapshot` VARCHAR(120) NN | Propuesto `purchase_order.supplier_name_snapshot`                                                | **P**. No resolver por JOIN al nombre actual ni normalizar destructivamente.                                                                                                                                |
| `created_by_user_id` UUID NN             | Propuesto `purchase_order.created_by_user_id` → `pos_user`                                       | **P**. Preservar actor original y pertenencia a organización. No usar usuario del import.                                                                                                                   |
| `issued_by_user_id` UUID N               | Propuesto `purchase_order.issued_by_user_id` → `pos_user`                                        | **P**. NULL en borrador/no emitida; conservarlo al cancelar una orden emitida.                                                                                                                              |
| `cancelled_by_user_id` UUID N            | Propuesto `purchase_order.cancelled_by_user_id` → `pos_user`                                     | **P**. Actor solo cuando exista cancelación; no inferirlo del creador.                                                                                                                                      |
| `version` BIGINT NN, default 0           | `purchase_order.expected_version`                                                                | **C/T**. Proponer conservar contador actual no negativo como revisión persistida y CAS. No confundirlo con la versión que envía un request ni resetearlo a 0. Cerrar semántica/nombre antes de implementar. |
| `created_at` TIMESTAMPTZ NN              | `purchase_order.created_at`                                                                      | **C**. Instante histórico, no fecha del import.                                                                                                                                                             |
| `updated_at` TIMESTAMPTZ NN              | Propuesto `purchase_order.updated_at`                                                            | **P**. Mantener última actualización; `created_at`/`ordered_at` no la sustituyen.                                                                                                                           |
| `issued_at` TIMESTAMPTZ N                | `purchase_order.ordered_at`                                                                      | **C/T**. Renombre propuesto del instante de emisión, conservando NULL e historial tras cancelación. No representa recepción.                                                                                |
| `cancelled_at` TIMESTAMPTZ N             | `purchase_order.cancelled_at`                                                                    | **C**. Conservar instante o NULL; nunca usarlo como cierre parcial.                                                                                                                                         |

La existencia de `supplier` y `pos_user` no resuelve la ausencia de columnas
históricas de cabecera. Auditoría separada tampoco sustituye automáticamente
campos necesarios para reconstruir la respuesta de la orden. [N12 §2; S-DTO-PO]

### 3.2 `purchase_order_lines` → `purchase_order_line` — 12/12

Origen: S23 L63–85; S-PL campos, constructor y validadores.
Destino físico de referencia: **N10 §16.40**.

| Columna origen / tipo                | Destino Rust/local                                 | Certeza y regla de preservación                                                                                                                                            |
| ------------------------------------ | -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id` UUID NN                         | `purchase_order_line.id`                           | **C**. Preservar UUID referenciado por recepciones.                                                                                                                        |
| `organization_id` UUID NN            | Propuesto `purchase_order_line.organization_id`    | **P** respecto de la ficha. Mantener scope explícito consistente con orden/artículo; N15 §5–6 respalda el scope, no completa silenciosamente §16.40.                       |
| `purchase_order_id` UUID NN          | `purchase_order_line.purchase_order_id`            | **C**. Misma orden; conservar integridad con organización.                                                                                                                 |
| `inventory_item_id` UUID NN          | `purchase_order_line.item_id`                      | **C/T**. Renombre de referencia al mismo InventoryItem, no SKU.                                                                                                            |
| `line_number` INTEGER NN >0          | Propuesto `purchase_order_line.line_number`        | **P**. Preservar posición/número, unicidad por orden y huecos existentes; no renumerar al importar.                                                                        |
| `item_code_snapshot` VARCHAR(64) NN  | Propuesto `purchase_order_line.item_code_snapshot` | **P**. Código capturado; `item_id`/SKU actual no lo reemplazan.                                                                                                            |
| `item_name_snapshot` VARCHAR(120) NN | Propuesto `purchase_order_line.item_name_snapshot` | **P**. Nombre histórico, incluso si catálogo cambió.                                                                                                                       |
| `unit_snapshot` VARCHAR(16) NN       | Propuesto `purchase_order_line.unit_snapshot`      | **P**. UNIT/GRAM/MILLILITER; acompaña QuantityExact para interpretar pedidos y recepciones históricas.                                                                     |
| `quantity` NUMERIC(19,6) NN >0       | `purchase_order_line.ordered_quantity_exact`       | **C/T**. Cantidad solicitada exacta, seis decimales históricos, misma unidad; no cantidad pendiente ni recibida.                                                           |
| `unit_cost` NUMERIC(19,4) N, >=0     | `purchase_order_line.unit_cost_exact`              | **C/!**. Campo existe pero F1–10 exige NN; Java/F1–12 conservan NULL. Costo pactado exacto, no costo efectivo/promedio. Propuesta de costo desconocido en §6, no adoptada. |
| `created_at` TIMESTAMPTZ NN          | Propuesto `purchase_order_line.created_at`         | **P**. No reemplazar por fecha de orden/recepción.                                                                                                                         |
| `updated_at` TIMESTAMPTZ NN          | Propuesto `purchase_order_line.updated_at`         | **P**. Mantener fecha propia aunque no aparezca en DTO de línea.                                                                                                           |

Origen impone una partida por `(purchase_order_id, inventory_item_id)` y número
único por orden; no perder esas reglas por el renombre a `item_id`. La partida
no tiene moneda ni total propios: se trata por separado en §4. [S23 L76–84]

### 3.3 `purchase_receipts` → `purchase_receipt` — 9/9

Origen: S25 L28–51; S-RH campos/create.
Destino físico de referencia: **N10 §16.17**.

| Columna origen / tipo                | Destino Rust/local                      | Certeza y regla de preservación                                                                                                                                 |
| ------------------------------------ | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id` UUID NN                         | `purchase_receipt.id`                   | **C**. Identidad del hecho inmutable, misma en replay.                                                                                                          |
| `organization_id` UUID NN            | `purchase_receipt.organization_id`      | **C**. Debe coincidir con orden/sucursal/actor.                                                                                                                 |
| `branch_id` UUID NN                  | `purchase_receipt.branch_id`            | **C**. Misma sucursal destino de la orden; no transferir stock al importar.                                                                                     |
| `purchase_order_id` UUID NN          | `purchase_receipt.purchase_order_id`    | **C**. Mantener enlace compuesto organización/sucursal/orden.                                                                                                   |
| `idempotency_key` VARCHAR(128) NN    | `purchase_receipt.command_id`           | **C/T**. Preservar texto, no exigir UUID. Scope Java `(organization_id, purchase_order_id, key)`; §16.17 no basta para asumir unicidad global. Protocolo en §7. |
| `payload_fingerprint` VARCHAR(64) NN | `purchase_receipt.payload_digest`       | **C/T**. Preservar huella y semántica canónica Java; renombrar no autoriza rehashear otro formato. Dato interno ausente del DTO, no del origen.                 |
| `received_by_user_id` UUID NN        | `purchase_receipt.received_by`          | **C/T**. Mismo actor autenticado → `pos_user`; no actor del import.                                                                                             |
| `received_at` TIMESTAMPTZ NN         | `purchase_receipt.received_at`          | **C**. Instante de negocio fijado por servidor.                                                                                                                 |
| `created_at` TIMESTAMPTZ NN          | Propuesto `purchase_receipt.created_at` | **P**. Instante de persistencia propio; no colapsarlo con received_at.                                                                                          |

### 3.4 `purchase_receipt_lines` → `purchase_receipt_line` — 9/9

Origen: S25 L53–78; S-RL campos/create/requireQuantity.
Destino físico de referencia: **N10 §16.41**.

| Columna origen / tipo                   | Destino Rust/local                                  | Certeza y regla de preservación                                                                                                                                   |
| --------------------------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id` UUID NN                            | `purchase_receipt_line.id`                          | **C**. Misma identidad que enlaza el ledger legado.                                                                                                               |
| `organization_id` UUID NN               | Propuesto `purchase_receipt_line.organization_id`   | **P** respecto de la ficha; preservar scope explícito conforme a N15 §5–6.                                                                                        |
| `branch_id` UUID NN                     | Propuesto `purchase_receipt_line.branch_id`         | **P** respecto de la ficha; misma sucursal que receipt/orden/movimiento.                                                                                          |
| `purchase_receipt_id` UUID NN           | `purchase_receipt_line.receipt_id`                  | **C/T**. Renombre, misma cabecera inmutable.                                                                                                                      |
| `purchase_order_id` UUID NN             | Propuesto `purchase_receipt_line.purchase_order_id` | **P**. Mantener vínculo explícito que participa en FKs legadas; derivarlo por padres sería una transformación por aprobar y verificar, no eliminación silenciosa. |
| `purchase_order_line_id` UUID NN        | `purchase_receipt_line.purchase_line_id`            | **C/T**. Mismo UUID de partida; única pareja receipt/partida.                                                                                                     |
| `inventory_item_id` UUID NN             | Propuesto `purchase_receipt_line.item_id`           | **P**. Mantener artículo explícito y comprobar igualdad con partida/movimiento; derivación por partida no es un destino físico ya confirmado.                     |
| `quantity_received` NUMERIC(19,6) NN >0 | `purchase_receipt_line.accepted_quantity_exact`     | **C/T**. Cantidad positiva que entró a stock, exacta; no implica dato histórico de rechazos.                                                                      |
| `created_at` TIMESTAMPTZ NN             | Propuesto `purchase_receipt_line.created_at`        | **P**. Conservar timestamp de línea, no fecha recalculada.                                                                                                        |

## 4. Campos destino sin hecho directo de origen

Esta es la diferencia entre **dato faltante**, **dato derivable** y **hecho
nuevo**. No confundir default destino con evidencia histórica.

| Destino / fuente física                                                              | Disponibilidad en Java V23–V25                                                              | Tratamiento propuesto y bloqueo                                                                                                                                                                                                                                                                |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `purchase_order_line.currency` — N10 §16.40                                          | No columna en línea; sí moneda histórica NN en orden.                                       | Derivación exacta desde `purchase_orders.currency`, con trazabilidad. No moneda actual ni default MXN. No resuelve órdenes sin líneas ni falta de moneda de cabecera.                                                                                                                          |
| `purchase_order_line.line_total_exact` — N10 §16.40, NN                              | No total almacenado; hay cantidad y costo pactado opcional.                                 | Con costo conocido puede proponerse producto exacto, etiquetado derivado/pactado, no factura ni valoración. Escalas 6×4 pueden producir 10 decimales y rango superior al de cada operando: validar límites sin redondeo silencioso. Si costo NULL, total desconocido; conflicto NN.            |
| `purchase_receipt_line.unit_cost_exact` — N10 §16.41, NN                             | **No existe costo propio de recepción**, tampoco en request/response.                       | No copiar automáticamente costo pactado, catálogo actual o MWAC. Incluso con costo pactado informado falta probar costo efectivo de entrada. Requiere política histórica aprobada; no hay mapping fiel completo hoy.                                                                           |
| `purchase_receipt_line.rejected_quantity_exact` — N10 §16.41                         | No rechazos ni cantidad entregada total.                                                    | No afirmar rechazo cero como hecho. Si se aprueba 0 como convención técnica de «sin rechazo registrado en el modelo legado», conservar procedencia/limitación y excluir afirmaciones sobre entrega real. Sin esa decisión/representación, bloquear; no usar el default para inventar historia. |
| `purchase_receipt_line.movement_id` — N10 §16.41                                     | Está en la relación inversa: `inventory_movements.purchase_receipt_line_id`.                | Resolver por enlace exacto y tipo PURCHASE_RECEIPT, no por cercanía temporal ni artículo solamente; exigir cardinalidad uno para hechos operativos importados. Cero/múltiples: inconsistencia y fail-fast, no fabricar movimiento.                                                             |
| `purchase_receipt.status` — N10 §16.17                                               | No columna de estado; Java persiste recepciones confirmadas append-only.                    | Proponer POSTED como traducción del hecho confirmado, no inferencia de pago/valoración CONFIRMED. No crear DRAFT de receipt legado.                                                                                                                                                            |
| `purchase_order.closed_at`, `close_reason`; estado CLOSED_PARTIAL — N10 §16.16       | No existen cierre de remanente ni motivo en estas tablas.                                   | No generarlos para parciales/cancelados legados. Se preservan sus estados; capacidades nuevas se separan en §5.                                                                                                                                                                                |
| Motivo de cancelación exigido por N15 §4                                             | Java cancel recibe versión/actor; no motivo en estas cuatro tablas.                         | No inventar motivo «migrado», ni usarlo para rechazar un CANCELLED históricamente válido. Compatibilidad histórica por aprobar; no equivale a dispensar el motivo en operaciones nuevas.                                                                                                       |
| `valuation_entry` — N10 §16.18                                                       | No ledger de valor/costing policy/estado de valoración en este flujo.                       | No generar asientos CONFIRMED ni PROVISIONAL con importe ficticio. Política versionada (`CostingPolicy`) carece de ficha física según auditoría §3; no crear tabla mínima para resolverlo.                                                                                                     |
| DomainEvent, outbox, instalación/generación/secuencia, command dedup — N10 §16.21–23 | No hechos equivalentes producidos por `PurchaseReceiptService.post`. Sí auditoría separada. | El journal Rust futuro es obligación diferente del import histórico; definir baseline/procedencia y compatibilidad. No inventar eventos firmados retrospectivos, secuencias o comandos de emisión/cancelación Java.                                                                            |

Los acumulados recibido/pendiente **sí son calculables**: por partida,
`received = SUM(quantity_received)` y `pending = quantity - received`, dentro
del mismo scope. No son columnas fuente ni se derivan del estado de cabecera.
Conservar cada recepción, no reemplazarlas por un único acumulado.
[S-RS `receiptStatus`/`receivedByLine`; S-DTO-STATUS]

### 4.1 Ledger físico y proyección: preservar, no volver a ejecutar

S-RS `applyStock` crea **un** movimiento positivo PURCHASE_RECEIPT por línea,
añade su cantidad al balance efectivo bloqueado y guarda `resulting_quantity`.
S25 añade FK compuesta movimiento→línea por organización/sucursal/artículo,
unicidad parcial por línea y triggers append-only de recepción. S17 protege el
ledger contra UPDATE/DELETE. Importar las recepciones no debe ejecutar de nuevo
ese comando sobre movimientos/balances ya importados.

| Hecho del ledger relacionado                                            | Destino y certeza                                                                           | Condición de paridad                                                                                                                                                                             |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `inventory_movements.id`, `organization_id`, `branch_id`, `occurred_at` | **C**, N10 §16.15 campos homónimos.                                                         | Preservar IDs/scope/instante propios, no reconstruir desde receipt.                                                                                                                              |
| `inventory_item_id`, `quantity_delta`, `type=PURCHASE_RECEIPT`          | **C/T**, `item_id`, `delta_quantity_exact`, `movement_type` en §16.15.                      | Igual delta positivo a cantidad recibida; crosswalk explícito de enum Java a RECEIPT si ese es el valor final Rust. No reducir todos los tipos del ledger a recepción.                           |
| `purchase_receipt_line_id`                                              | **C/T**, `source_type` + `source_id` en §16.15; relación inversa a `movement_id` en §16.41. | Proponer origen tipado **línea de recepción**, no cabecera. Mantener unicidad por efecto y enlace exacto; TypedRef no reemplaza una FK real. No UNIQUE global de source_id.                      |
| `actor_user_id`, `resulting_quantity`, `reason`                         | **P**, conservación explícita propuesta en `inventory_movement` (nombres a aprobar).        | §16.15 no enumera esos destinos. No perder actor/balance histórico por conservar solo delta; para receipt reason es NULL del flujo, no motivo inventado.                                         |
| `sale_id`                                                               | No aplica al movimiento de recepción: NULL exigido por S25.                                 | No enlazar recepción a venta ni borrar referencias de movimientos SALE_CONSUMPTION al importar el resto del ledger.                                                                              |
| `origin_event_id` destino                                               | **C**, §16.15, pero sin equivalente en este hecho Java.                                     | No inventar correlación con DomainEvent. La política de baseline/journal debe resolverla.                                                                                                        |
| `branch_inventory.quantity_on_hand`                                     | **C/T**, `branch_stock.on_hand_quantity_exact` en §16.14.                                   | Reconciliar por organización/sucursal/artículo contra ledger completo y baseline verificable, no solo sumar receipts. Versiones y demás hechos de proyección requieren su mapping de inventario. |

**Límite de las garantías SQL:** la unicidad parcial impide _más de uno_, no
obliga por sí sola a que _exista uno_. Las FKs fijan pertenencia, pero no prueban
que delta = cantidad recibida, que la suma no exceda lo pedido, ni que el artículo
de receipt line sea el de la PO line (la FK a esa partida no incluye artículo).
Eso lo construye/valida el servicio; el import debe verificarlo también.
Tampoco el CHECK de estado demuestra por sí solo que todas las partidas estén
completas. [S25 L63–104, L126–155; S-RS `validateRequestedLines`, `rejectOverReceipt`,
`applyStock`, `nextStatus`; T06]

## 5. Estados y transiciones: paridad frente a capacidades nuevas

| Operación                                         | AS-IS Java y efecto                                                                                                     | Destino/condición                                                                                                                                                                      |
| ------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Crear orden                                       | → DRAFT; código/moneda/actor/snapshots; sin stock.                                                                      | Preservar; estado servidor, no PATCH arbitrario.                                                                                                                                       |
| Editar/agregar/quitar partida o cambiar proveedor | Solo DRAFT y versión válida. Mutación real incrementa versión del agregado y audita; no-op no cambia versión/auditoría. | Preservar triestado de costo y reglas de snapshots. No imponer costo conocido.                                                                                                         |
| Emitir                                            | DRAFT → ORDERED; ≥1 línea, supplier/items activos, unidad compatible, actor/fecha. Sin stock ni costo requerido.        | Estados y guards de N15 §4; costo desconocido sigue siendo hecho válido de origen.                                                                                                     |
| Cancelar sin recepción                            | DRAFT u ORDERED → CANCELLED; conserva emisión si ya existía; sin stock.                                                 | Mantener historia, no convertirlo a CLOSED_PARTIAL. N15 añade razón/comando: faltan hechos legados, no rellenarlos.                                                                    |
| Primera recepción                                 | ORDERED → PARTIALLY_RECEIVED o RECEIVED según todas las líneas.                                                         | Cantidades positivas, acumulado ≤ pedido, versión y scope; costo desconocido no bloquea Java.                                                                                          |
| Recepción posterior                               | PARTIALLY_RECEIVED → PARTIALLY_RECEIVED o RECEIVED.                                                                     | Repetir parciales mientras haya pendiente; RECEIVED solo si **cada** línea completa su cantidad.                                                                                       |
| Cancelar después de recepción                     | Servicio rechaza `PURCHASE_ORDER_HAS_RECEIPTS`, incluso si estado dice otra cosa.                                       | No deshacer stock ni cambiar a CANCELLED. `PurchaseOrder.cancel` aislado no contiene toda esta guarda: portar frontera de servicio, no solo entidad.                                   |
| Nueva recepción en DRAFT/CANCELLED/RECEIVED       | Rechazada con error de estado; replay idéntico se resuelve antes (§7).                                                  | Conservar distinción de errores, no reabrir terminales.                                                                                                                                |
| Cerrar remanente                                  | No existe en enum, API ni tablas Java.                                                                                  | N10 §16.16 y N15 §3–4 contemplan PARTIALLY_RECEIVED → CLOSED_PARTIAL con permiso, razón y fecha; no revierte cantidades recibidas. Es capacidad nueva, no traducción de una fila Java. |
| Receipt confirmado                                | Java no tiene enum propio; tras commit es inmutable.                                                                    | N15/N10: POSTED terminal; corrección futura por compensación explícita, no UPDATE/DELETE. No supone que haya sido valorado/pagado.                                                     |

SQL final exige emisión con actor/fecha en ORDERED/PARTIALLY_RECEIVED/RECEIVED y
cancelación con actor/fecha en CANCELLED. DRAFT no tiene ninguno. El CHECK de
CANCELLED no impone por sí mismo todas las condiciones de la transición de
servicio. Fechas de emisión existentes no se limpian al cancelar. [S25 L143–155;
S-PO `issue`/`cancel`/`applyReceipt`; S-OS `cancel`/`requireNoConfirmedReceipts`]

## 6. Propuesta de costo desconocido — NO ADOPTADA

Se recomienda evaluar la alternativa B de ADR-0021 para preservar historia, no
se aprueba aquí. A/C/D siguen abiertas. **Recomendación inequívoca: no NULL→0**,
ni catálogo→costo histórico, ni promedio→precio pactado/efectivo.

### 6.1 Semántica propuesta, no cambio físico autorizado

- Distinguir costo pactado **desconocido**, **conocido cero** y **conocido positivo**
  en DRAFT, ORDERED, PARTIALLY_RECEIVED, RECEIVED y CANCELLED. NULL expresa falta
  de dato, no mercancía gratuita. Un `Option<UnitCostExact>` sería una posible
  representación Rust del costo pactado, no una decisión de API/DDL.
- Para PO line, proponer `unit_cost_exact` nullable **y** `line_total_exact`
  desconocido cuando el costo lo sea. Solo calcular total pactado exacto con
  ambos operandos conocidos. Cambiar una sola nulabilidad no resuelve §16.40.
- Para receipt line, proponer representar costo efectivo **no documentado** cuando
  no exista fuente histórica independiente. El costo pactado conocido puede
  conservarse como referencia, pero no elevarse a costo efectivo sin política y
  procedencia aprobadas. Cambiar nullable solo la PO no resuelve §16.41.
- Evaluar estado/procedencia de conocimiento/valoración que separe «sin evidencia
  de costo efectivo» de «valoración provisional con estimación documentada» y de
  «confirmada». Son conceptos propuestos, **no nuevos campos/enums/tablas
  aprobados**. La representación histórica puede requerir alternativa D.
- Un asiento provisional también necesita base numérica y regla justificadas.
  N10 §16.18 exige `amount_exact` y política NN: no insertar NULL ni un asiento
  PROVISIONAL de cero para aparentar cumplimiento. Resolver expresamente cómo
  registrar la valoración pendiente sin asiento económico falso.
- Costo tardío se regularizaría con evidencia, política/versionado y hechos
  económicos compensatorios/correlacionados; no editando receipt/movement ni
  duplicando la cantidad. Falta aprobar su representación y reconciliación.

### 6.2 MWAC sin inventar costo

**Confirmado como baseline, no implementación Java:** N13 §7–8/§10/§13 y
ratificación final adoptan promedio ponderado móvil por sucursal/artículo y
separación de ledger físico/económico. Ese baseline no autoriza rellenar costos
legados ni elegir arbitrariamente una valoración inicial.

Si existe saldo **completamente valorado**, misma moneda/unidad, Q > 0 y entrada
aceptada q con costo efectivo conocido c, entonces conceptualmente:

- `Q_nuevo = Q + q`;
- `V_nuevo = V + q × c`;
- `MWAC_nuevo = V_nuevo / Q_nuevo`, bajo precisión y política versionadas.

El precio pactado por sí solo **no acredita c**. Si c es desconocido, Q puede
conocerse, pero el valor total nuevo no; conservar V viejo y dividirlo por Q
nuevo infravalora stock e inventa un promedio. Sumar solo entradas con costo y
presentar ese subtotal como valor total también es incorrecto. Una entrada
posterior conocida no subsana por sí sola todas las unidades históricas sin
valoración; salidas y compensaciones intermedias importan.

Con costo efectivo cero **documentado**, q×c=0 es válido y distinto de falta de
costo. Si Q=0 no dividir; si Q<0 por excepción explícita, no producir un promedio
confirmado ficticio. Los saldos negativos legados no se corrigen a cero durante
paridad (T07 contiene recepción desde saldo negativo). Residuo V con Q=0,
precisión de división, regularización y política de negativos necesitan reglas
trazables, no defaults de import. [N13 §8; S-RS `applyStock`; T07]

**Mientras no se apruebe la resolución:** exportar fielmente NULL/snapshots;
detener el import afectado antes de efectos o revertir toda su unidad
transaccional. No habilitar unilateralmente recepción Rust «solo cantidad» si
el baseline exige stock+valor atómicos. La separación conceptual Q/V no es una
exención del gate ni autorización para marcar compras conformes.

## 7. Atomicidad, idempotencia y reintentos

### 7.1 Garantías del flujo Java que deben preservarse

`PurchaseReceiptService.post` es una transacción: normaliza clave; bloquea orden
por scope; busca receipt existente; solo para nueva operación valida estado,
versión, sucursal, líneas y acumulados; bloquea/marca artículos en orden estable;
persiste cabecera/líneas; bloquea balances y suma stock; inserta ledger; cambia
estado/versión de orden; registra auditoría; confirma todo o nada. La recepción
no exige proveedor activo en ese camino: no añadir esa guarda por analogía con
emisión. [S-RS L122–189, `applyStock`, `validateItems`]

- Falla de auditoría o rango de stock revierte cabecera, líneas, balance,
  movimientos, estado, versión y consumo de clave. Tests T05 verifican reintento
  posterior con la misma clave; no es una promesa de esta auditoría runtime.
- Java serializa por orden y por balances/artículos; una misma clave concurrente
  no duplica efectos y claves distintas con la misma versión no permiten dos
  mutaciones. No trasladar literalmente SELECT FOR UPDATE a SQLite: conservar
  resultado mediante transacción/escritor/CAS y reintentos acotados adecuados al
  motor, con FKs habilitadas por conexión. [T08, T09; N10 §3/§10]
- La nueva frontera Rust definida en N15 §4 incluye stock, valoración válida,
  auditoría y eventos; N10 §7/§16.21–23 aporta journal/dedup/outbox. Reintento o
  fallo debe abarcar también esos efectos cuando se implementen, no solo stock.
  No se atribuyen esos asientos/eventos a la implementación Java actual.

### 7.2 Identidad y digest: compatibilidad, no solo renombre

1. Java recorta clave, exige no vacía y hasta 128 caracteres; unicidad por
   `(organization_id, purchase_order_id, idempotency_key)`. Mismo texto puede ser
   válido en otra orden/organización. El scope de `command_dedup` futuro debe
   codificar esa identidad sin colisiones y sin reducir aislamiento de sucursal.
2. La huella Java ordena por UUID de PO line (`UUID.compareTo`), concatena
   `uuid:cantidad_con_6_decimales` con `|` y calcula SHA-256 UTF-8 hexadecimal.
   Conservar huella original y versionar compatibilidad del algoritmo; no asumir
   que ordenar UUID como texto en Rust produce siempre el mismo orden Java.
   Variantes `1`, `1.0`, `1.000000` y permutaciones de líneas equivalen.
3. `expectedVersion` no participa en la huella: replay se resuelve **antes** de
   guards de estado/versión, sin omitir autenticación/autorización del controller.
   Una repetición idéntica puede usar versión obsoleta o llegar tras RECEIVED.
   Para listas válidas sin duplicados, una huella distinta con misma clave da
   409 `PURCHASE_RECEIPT_IDEMPOTENCY_CONFLICT`, sin efectos nuevos.
   **Desviación AS-IS:** `replay()` usa `putIfAbsent` y descarta entradas repetidas
   posteriores; tras `[(L,1)]`, el replay `[(L,1),(L,2)]` puede devolver 200 al
   ignorar la segunda cantidad (servicio L250–267). El camino nuevo sí rechaza
   duplicados. Ya estaba registrado en `rust-parity-baseline.md` §6.2 punto 9.
   No perpetuar el defecto automáticamente ni prometer aquí rechazo universal:
   decidir compatibilidad explícita y añadir regresión diferencial con duplicados
   iguales/distintos en ambas posiciones, sin cambiar Java en este incremento.
4. Replay conserva receipt/líneas originales y no incrementa versión, stock ni
   auditoría. **La respuesta incluye la orden vigente**, no necesariamente una
   copia byte a byte de la respuesta HTTP inicial si hubo otras recepciones.
   `replay` construye detalle con la orden cargada en esa petición.
5. El import debe conservar los registros de identidad/digest, no llamar al
   endpoint de recepción. Reejecutar el mismo bundle debe ser idempotente o
   fail-fast sin duplicar efectos (N12 PAR-09); ID igual con contenido divergente
   no se resuelve sobrescribiendo historia. Ese protocolo de import es distinto
   del replay de comandos de negocio y aún necesita implementación/pruebas.

Fuentes: S25 L33–39/L174–177; S-RS `post`, `replay`, `fingerprint`,
`requireIdempotencyKey`; S-API-R `post`; T04/T08.

## 8. Pruebas inspeccionadas y aceptación pendiente

**Estado de evidencia:** los tests siguientes se leyeron como código; no se
ejecutaron suites Java/Rust, DDL ni import en 0c. Su presencia no equivale a CI
verde. Se requiere repetir comportamientos en el destino y comparar fixtures
sintéticos con procedencia verificable, no datos ni documentos confidenciales.

| Evidencia fuente exacta                                                                                                                                                                                       | Qué cubre                                                                                    | Aceptación requerida para Rust/import                                                                                                                                                                                                                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T01 `createReturnsDraftWithServerOwnedCurrencyStatusAndSupplierSnapshots`, `addLineCapturesSnapshotsAndForbidsDuplicateItem`, `snapshotSurvivesCatalogRename`                                                 | Moneda servidor, snapshots y unicidad.                                                       | Comparar todos los campos de §3, incluidos campos no expuestos en DTO, actores/fechas/versiones. Cambiar catálogo/moneda actual no altera historia; cubrir proveedor renombrado y cambio de proveedor en DRAFT.                                            |
| T01 `updateLineCanKeepSetZeroAndClearUnitCost`; T02 helper `addLine` + `issueTransitionsDraftToOrderedStampsActorAndTimeAndAudits`; T03 helper `issuedOrder` + `fullReceiptPostsStockCompletesOrderAndAudits` | NULL/cero/PATCH, emisión y recepción sin costo en fixtures.                                  | Matriz explícita NULL/cero/positivo en DRAFT, ORDERED, PARTIALLY_RECEIVED y RECEIVED; NULL de total/receipt según resolución aprobada. Incluir CANCELLED con y sin emisión; no imponer costo al pasado.                                                    |
| T02 `cancelDraftSetsCancelledWithoutIssueDataAndAudits`, `cancelOrderedPreservesIssueHistory`; T03 `cancelIsRejectedWhenReceiptsExist`                                                                        | Actores/fechas e irreversibilidad.                                                           | Mismos estados/errores/versión, sin stock por emitir/cancelar. CLOSED_PARTIAL se prueba como capacidad nueva, nunca remapeando Java.                                                                                                                       |
| T03 `partialReceiptLeavesPendingAndSecondReceiptCompletes`, `overReceiptIsRejectedAtomically`; T07 `receivedStatusRequiresEveryLineComplete`, `cumulativeOverReceiptAcrossTwoReceiptsIsRejected`              | Parciales, pendientes y sobrerrecepción acumulada.                                           | Dos o más líneas; 6 decimales, no compensar sobrante de una con faltante de otra; acumulados iguales y sin pérdida de receipts individuales.                                                                                                               |
| T04 (cinco métodos), T03 `reusedKeyWithDifferentPayloadConflicts`                                                                                                                                             | Orden/escala equivalentes, replay terminal/versión vieja, aislamiento de claves, conflicto.  | Golden digest Java↔Rust, UUID con bits altos, límites/espacios de clave, respuesta con orden vigente tras una recepción posterior; efectos exactamente una vez.                                                                                            |
| T05 `auditFailureRollsBackHeaderLinesStockOrderAndAudit`, `lateStockRangeFailureRollsBackHeaderLinesAndBalances`                                                                                              | Rollback tardío y reutilización de clave tras fallo.                                         | Inyectar fallo en cada etapa, incluido ledger de valor/journal/outbox destino, y corte antes/después de commit. Nada parcial ni clave consumida sin resultado durable.                                                                                     |
| T06 `duplicateMovementPerReceiptLineIsRejectedByPostgres`, `receiptBranchMustMatchItsOrder`, pruebas de referencias ajenas e inmutabilidad; T10                                                               | Constraints PostgreSQL y append-only.                                                        | Constraints/guards equivalentes SQLite: org/sucursal/artículo/partida correctos, rechazo UPDATE/DELETE de hechos, cero/múltiples movimientos detectados en preflight. No atribuir a SQL invariantes que solo valida servicio.                              |
| T08 `simultaneousIdenticalReceiptsCommitExactlyOnce`, `simultaneousDistinctReceiptsSerializeOnOrderLock`; T09 métodos de concurrencia                                                                         | Un commit/replay, CAS, venta/ajuste/cancelación/desactivación concurrentes y orden de locks. | Dos conexiones reales SQLite; busy/retry acotado, crash y CAS sin lost update ni doble ledger/valor. Conciliar stock completo tras carreras.                                                                                                               |
| T07 validación de precisión/rango y `negativeStartingBalanceRecordsDeltaAndEffectiveResulting`                                                                                                                | Cantidad inválida rechazada; delta positivo sobre saldo negativo efectivo.                   | Round-trip UUID/instantes/decimales máximos, cantidades >0 con hasta 6 decimales y costo >=0 hasta 4; sin float ni redondeo para encajar. Preservar negativos históricos sin afirmar MWAC confirmado.                                                      |
| S23/S25 + §3/§4 de este contrato                                                                                                                                                                              | Cobertura 47/47 y brechas de destino.                                                        | Control automático de columnas, conteos/checksums, FKs y snapshots; ningún campo silenciosamente descartado. Si falta destino o costo efectivo, fail-fast/rollback hasta aprobación. PAR-01/02/03/04/07/08/09 aplicables, no verdes por documentación.     |
| N13 §8 y §6 de este contrato                                                                                                                                                                                  | MWAC separado del ledger físico; no motor de valor Java equivalente.                         | Vector conocido 10×20 + 10×30 → Q20/V500/promedio25; cero efectivo documentado; entrada desconocida antes/después de conocida; stock negativo/Q0/residuo; costo tardío compensatorio sin segunda cantidad. No inventar saldo inicial V desde Q o catálogo. |

**Condición de cierre:** aprobar resolución de PHY-PAR-03/04/05 y destinos P,
reconciliar F1–10/F1–12/F4–21 mediante change control, definir política/representación
histórica de costo y hechos ausentes, y ejecutar gates con evidencia real.
Mantener ADR-0021 PROPOSED hasta decisión explícita de responsables autorizados.
Este contrato no resuelve las cinco dependencias físicas faltantes ni el conteo
82/83, y no autoriza DDL para «completar» el modelo.

## 9. Fuentes con rutas y secciones exactas

Rutas relativas a la raíz del repositorio. Prefijos exactos para evitar ambigüedad:

- **SQL:** `backend/src/main/resources/db/migration/`.
- **J:** `backend/src/main/java/com/hiqnet/pos/`.
- **T:** `backend/src/test/java/com/hiqnet/pos/`.

### 9.1 Reglas, ADR y auditoría previa

- `AGENTS.md`, apartados **Contrato de API y documentación**, **Branching** y
  **Pull requests**: autoridad OpenAPI y límites de actuación.
- `docs/01-architecture/adr/0021-frozen-physical-parity-conflicts.md`, **Estado**,
  **Contexto y hechos comprobados** (PHY-PAR-03/04/05), **Default operativo
  mientras no exista aprobación**, **Alternativas propuestas, no aprobadas** y
  **Criterios para resolver y revisar el estado**.
- `docs/05-development/rust-physical-audit.md`, **§3 Cinco dependencias sin
  definición física tabular**, **§4 Evidencia unit_cost**, **§5 Snapshots y
  destinos de paridad incompletos**, **§6 Impacto sobre 0b**.

### 9.2 SQL, dominio, servicios y DTOs

| ID           | Ruta exacta bajo prefijo / sección inspeccionada                                                                                                                                                                                                                          |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S23          | SQL `V23__create_purchasing_purchase_order_foundation.sql`, L12–85 tablas/constraints; L103–112 comentarios de moneda, costo y unidad.                                                                                                                                    |
| S24          | SQL `V24__inventory_item_base_unit_lock.sql`, L1–13 coordinación, L15–39 marcador/backfill y L41–47 persistencia/recepción.                                                                                                                                               |
| S25          | SQL `V25__create_purchasing_purchase_receipts.sql`, §1–3 L14–78 scope/tablas; §4–6 L80–130 ledger/inmutabilidad/unicidad; §7 L132–155 estados; L170–179 semántica.                                                                                                        |
| S17          | SQL `V17__branch_inventory_stock_ledger.sql`, L2–48 balances, hechos, FKs y trigger append-only.                                                                                                                                                                          |
| S-PO         | J `purchasing/domain/PurchaseOrder.java`, L56–109 campos, `create`, `changeSupplier`, `requireDraft`, `registerLineMutation`, `issue`, `cancel`, `applyReceipt`, `onCreate`/`onUpdate`; `purchasing/domain/PurchaseOrderStatus.java`, enum e `isReceivable`.              |
| S-PL         | J `purchasing/domain/PurchaseOrderLine.java`, L55–90 campos, constructor, `updateQuantity`, `updateUnitCost`, `requireQuantity`, `requireUnitCost`, callbacks de fechas.                                                                                                  |
| S-RH         | J `purchasing/domain/PurchaseReceipt.java`, L35–60 campos, `create`, `onCreate`.                                                                                                                                                                                          |
| S-RL         | J `purchasing/domain/PurchaseReceiptLine.java`, L36–61 campos, `create`, `requireQuantity`, `onCreate`.                                                                                                                                                                   |
| S-OS         | J `purchasing/application/PurchaseOrderService.java`, `create` L116–141, `update` L200–230, `addLine`/`updateLine`/`removeLine` L243–367, `issue` L396–423, `cancel` L444–461, `lockActiveSupplier`/`lockIssuableItems` L477–534, `requireNoConfirmedReceipts` L659–666.  |
| S-RS         | J `purchasing/application/PurchaseReceiptService.java`, `post` L122–189, `receiptStatus` L235–248, `replay` L250–268, `applyStock` L270–308, validación/acumulados/estado L310–411, auditoría L417–431, guards/clave/fingerprint L439–524.                                |
| S-DEC        | J `purchasing/application/PurchaseOrderAmounts.java`, `requireQuantity`, `optionalUnitCost` L53–71, `formatQuantity`/`formatUnitCost` L80–91.                                                                                                                             |
| S-DTO-PO     | J `purchasing/api/PurchaseOrderResponse.java`, record/`from` L13–56; `PurchaseOrderLineResponse.java`, L12–42; `CreatePurchaseOrderLineRequest.java`, L13–29; `UpdatePurchaseOrderLineRequest.java`, L11–32; `PurchaseOrderController.java`, `updateLine` L255–287.       |
| S-API-R      | J `purchasing/api/PostPurchaseReceiptRequest.java`, record L15–24; `PostPurchaseReceiptLineRequest.java`, L12–20; `PurchaseReceiptResponse.java`, L12–37; `PurchaseReceiptLineResponse.java`, L12–30; `PurchaseReceiptController.java`, `post` L61–99 y lectura L110–188. |
| S-DTO-STATUS | J `purchasing/api/PurchaseOrderLineReceiptStatusResponse.java`, record/`from` L12–35.                                                                                                                                                                                     |
| S-JDBC       | J `inventory/stock/infrastructure/JdbcInventoryMovementRepository.java`, `insertBatch` L54–75 y `findHistory`/`map` L90–122.                                                                                                                                              |

### 9.3 Pruebas de origen (inspección, no ejecución)

| ID  | Ruta exacta bajo T / secciones                                                                                                                                                            |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T01 | `purchasing/api/PurchaseOrderApiTest.java`, L100–332 creación/snapshots/edición/costo; métodos citados en §8.                                                                             |
| T02 | `purchasing/api/PurchaseOrderTransitionApiTest.java`, helper `addLine` L75–81, emisión L112–137, guards L139–259, cancelación/historia L262–326.                                          |
| T03 | `purchasing/api/PurchaseReceiptApiTest.java`, fixture `issuedOrder` L68–103, recepción/parciales/over/replay/conflicto L151–294, estados/cancelación/versión L296–377.                    |
| T04 | `purchasing/api/PurchaseReceiptIdempotencyTest.java`, cinco tests L20–137.                                                                                                                |
| T05 | `purchasing/api/PurchaseReceiptRollbackTest.java`, dos tests L35–95 e inyección de auditoría L103–118.                                                                                    |
| T06 | `purchasing/api/PurchaseReceiptDbConstraintsTest.java`, métodos de unicidad/tipo L72–269; scope/inmutabilidad/inserts/replay L272–425.                                                    |
| T07 | `purchasing/api/PurchaseReceiptValidationTest.java`, tests L23–198: cantidades, precisión, duplicados, partida ajena, saldo negativo, todas las líneas, acumulados y entidades inactivas. |
| T08 | `purchasing/api/PurchaseReceiptConcurrencyTest.java`, tests L157–194.                                                                                                                     |
| T09 | `purchasing/api/PurchaseReceiptStockConcurrencyTest.java`, tests de secuencia/carreras L33–226.                                                                                           |
| T10 | `inventory/stock/InventoryMovementAppendOnlyTest.java`, tests L22–35; `purchasing/api/PurchaseOrderDbConstraintsTest.java`, tests L57–135.                                                |

### 9.4 Referencias normativas externas (sin reproducción del expediente)

| ID  | Referencia / sección exacta utilizada                                                                                                                                                                                                                                                                                                                           |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| N10 | [F1–10](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/edit), §2 tipos; §3 conexiones/transacciones SQLite; §6 inventario/compras/valor; §7 journal; §10 invariantes; §16.13–18 artículo/stock/ledger/orden/receipt/valoración; §16.21–23 eventos/outbox/dedup; §16.40–41 partidas; §17.15 supplier; ratificación F4–21 final. |
| N12 | [F1–12](https://docs.google.com/document/d/16hyT4XALY3DrAc50RUTx-fESzMI0c5612zTKphHRieM/edit), §2 filas purchase_orders, purchase_order_lines, purchase_receipts, purchase_receipt_lines e inventory_movements; §4 conversión exacta; §6 PAR-01..09; freeze final.                                                                                              |
| N13 | [F4–13](https://docs.google.com/document/d/1vL-yFrwenxeqbAjuWY9m5zOMhlMGADfVTDUeWaOa4OM/edit), §7 ADR-COST-001, §8 casos límite/pruebas, §10 baseline de costeo, §13 baseline consolidado, ratificación del owner y F4–21 finales. Párrafos históricos PROPOSED no anulan la ratificación; tampoco esta aprueba NULL→0.                                         |
| N15 | [F4–15](https://docs.google.com/document/d/12VYpuy-IznZufJpYiGqn-4BBi3JqeI0al3XCE5Gx7tU/edit), §2 lifecycle, §3 PurchaseOrder/PurchaseReceipt, §4 transiciones de ambos, §5/§5.1 scopes y §6 cambios físicos; ratificación F4–21 final.                                                                                                                         |

## 10. Validación documental de 0c

Se contrastó el inventario de columnas del Markdown contra los CREATE TABLE de
V23/V25 (47/47, sin duplicados), referencias locales y secciones fuente; se
revisó whitespace y el write set propio, limitado a este archivo. No se ejecutaron
pruebas funcionales Java/Rust, migraciones, importaciones ni cálculo MWAC en
runtime: el incremento es exclusivamente documental y no certifica gates ejecutables.

En el status final aparecieron cambios concurrentes en
`backend-rust/tools/migration-lab/scripts/verify_evidence.py` y el archivo nuevo
`backend-rust/tools/migration-lab/scripts/test_verify_evidence.py`. No se
modificaron ni se incorporan al write set o a la validación funcional de 0c.
