# Paridad Purchasing Java → Rust — Incremento 2A

**Estado: dominio implementado, sin persistencia ni API.** Este documento
inventaría lo que existe y lo que falta para cerrar el alcance de `v0.4.0`
(«Purchasing»). No autoriza DDL, import ni release.

## 1. Qué existe hoy en Java

17 endpoints operativos bajo `/api/v1`:

### Órdenes de compra (9)

| Método y ruta | Operación | Permiso |
| --- | --- | --- |
| `POST /purchase-orders` | Crear borrador con partidas | `purchasing.order.write` |
| `GET /purchase-orders` | Listar por sucursal | `purchasing.order.read` |
| `GET /purchase-orders/{id}` | Detalle con partidas | `purchasing.order.read` |
| `PATCH /purchase-orders/{id}` | Cambiar proveedor (solo borrador) | `purchasing.order.write` |
| `POST /purchase-orders/{id}/lines` | Añadir partida | `purchasing.order.write` |
| `PATCH /purchase-orders/{id}/lines/{lineId}` | Editar cantidad/costo | `purchasing.order.write` |
| `POST /purchase-orders/{id}/lines/{lineId}/remove` | Retirar partida | `purchasing.order.write` |
| `POST /purchase-orders/{id}/issue` | Emitir | `purchasing.order.issue` |
| `POST /purchase-orders/{id}/cancel` | Cancelar | `purchasing.order.cancel` |

### Recepciones (4)

| Método y ruta | Operación | Permiso |
| --- | --- | --- |
| `POST /purchase-orders/{id}/receipts` | Recepción atómica con stock | `purchasing.receipt.write` |
| `GET /purchase-orders/receipts` | Listar | `purchasing.receipt.read` |
| `GET /purchase-orders/receipts/{id}` | Detalle | `purchasing.receipt.read` |
| `GET /purchase-orders/receipt-status` | Estado acumulado por partida | `purchasing.receipt.read` |

### Proveedores (4)

| Método y ruta | Operación | Permiso |
| --- | --- | --- |
| `POST /suppliers` | Crear | `purchasing.supplier.write` |
| `GET /suppliers` | Listar | `purchasing.supplier.read` |
| `GET /suppliers/{id}` | Detalle | `purchasing.supplier.read` |
| `PATCH /suppliers/{id}` | Actualizar | `purchasing.supplier.write` |

## 2. Qué existe en Rust

| Capacidad | Estado | Evidencia |
| --- | --- | --- |
| Proveedor (slug inmutable, contacto acotado, email normalizado, snapshot) | **Dominio** | `pos-core::supplier` |
| Orden de compra: estados, emisión, cancelación, recepción acumulada | **Dominio** | `pos-core::purchasing` |
| Partida con snapshots y costo opcional | **Dominio** | `PurchaseOrderLine`, prueba 2A «desconocido ≠ cero» |
| Validación de recepción y sobre-recepción | **Dominio** | `validate_receipt_lines`, `OverReceiptViolation` |
| Unidad base inmutable frente a recepción | **Dominio** | `ensure_unit_unchanged` |
| Permisos `purchasing.*` | Catálogo físico | `0004` propuesto y `PermissionCode` validado |
| Persistencia SQLite | **No existe** | Sin tablas aprobadas |
| Casos de uso de aplicación | **No existe** | Sin puertos ni UoW |
| API/IPC | **No existe** | Sin handlers |
| Auditoría de compras | **No existe** | Sin eventos persistidos |

## 3. Bloqueos que impiden cerrar Purchasing

| Bloqueo | Por qué impide avanzar |
| --- | --- |
| ADR-0021 / CC21-03 (`unit_cost` NULL) | El esquema congelado no admite el estado Java; sin decisión no hay DDL |
| Cinco fichas padre | Las FK no pueden apuntar a tablas inexistentes |
| Bloque B/C sin aprobar | Proveedores y órdenes dependen de catálogo e inventario persistidos |
| Import sintético no autorizado | La paridad (`PAR-06/07`) exige target importado |
| Ledger de inventario aprobado | La recepción escribe stock; sin `0013`/`0014` aprobadas no hay dónde |

## 4. Estrategia propuesta para cerrar Purchasing

| Etapa | Contenido | Depende de | Resultado verificable |
| --- | --- | --- | --- |
| P1 | Aprobar `unit_cost` NULL y fichas padre | Titular | `CC21-03` aceptado, DDL de compras escribible |
| P2 | Migración candidata `0016_purchasing` + repositorios SQLite | P1 + bloques B/C | Clean install, FK, índices, reapertura |
| P3 | Casos de uso: crear/editar/emitir/cancelar orden, recepción atómica | P2 | CAS, auditoría, stock en la misma UoW |
| P4 | API/IPC + pantallas React | P3 | Paridad funcional con los 17 endpoints |
| P5 | Import sintético y casos `PAR-06/07`, `AS-06/07` | P4 | Evidencia normativa, no aprobación automática |

Ninguna etapa se ejecuta en 2A.

## 5. Regla de cierre para `v0.4.0`

`v0.4.0` no puede declararse preparado mientras:

1. Purchasing no tenga persistencia y API equivalentes.
2. El import no esté ejecutado y comparado.
3. Los casos `PAR-06/07` no estén verdes con evidencia.
4. La decisión `unit_cost` siga abierta.

Este documento no cambia `VERSION` ni el roadmap: `0.4.0-SNAPSHOT` continúa.
