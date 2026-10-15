# ADR-0015: Fundación de proveedores para Purchasing

- **Estado:** Aceptada
- **Fecha:** 2026-10-15

## Contexto

`v0.4.x Purchasing` comienza antes de modelar órdenes y recepciones. El sistema
necesita una fuente de datos maestros de proveedores compartida por las futuras
sucursales de una organización, sin introducir cuentas por pagar, pagos,
contabilidad, costos promedio ni movimientos de inventario de manera prematura.

Las convenciones vigentes asignan los catálogos administrativos a la
Organization, autorizan por permisos database-authoritative y preservan
trazabilidad mediante auditoría append-only.

## Decisión

1. `Supplier` es propiedad de una **Organization**; no pertenece a una Branch.
   Las órdenes futuras conservarán Organization + Branch y referenciarán un
   proveedor de la misma Organization.
2. Cada proveedor tiene un `code` comercial slug, normalizado en minúsculas,
   inmutable y único por Organization. No se impone unicidad global ni de
   `taxId`: su semántica legal y disponibilidad varían por negocio y país.
3. El dato mínimo es `code` y `name`. Razón social, identificador fiscal,
   contacto, correo, teléfono y dirección son opcionales y editables.
4. No hay `DELETE`. Un proveedor se retira con `status=INACTIVE`; su identidad
   queda disponible para referencias históricas futuras.
5. Los endpoints `POST`, `GET` colección/detalle y `PATCH` son
   organization-scoped, usan permisos independientes
   `purchasing.supplier.read` y `purchasing.supplier.write`, y exigen
   `expectedVersion` en escritura parcial.
6. Toda creación o actualización exitosa registra exactamente un evento de
   auditoría en la misma transacción. La metadata incluye solo código o nombres
   de campos modificados, nunca datos de contacto ni identificadores fiscales.
7. PostgreSQL mantiene la integridad final: FK a Organization, restricciones de
   formato/estado/textos, unicidad `(organization_id, code)` y la traducción de
   su constraint a `SUPPLIER_CODE_ALREADY_EXISTS` durante carreras concurrentes.

## Consecuencias

- El catálogo de proveedores puede reutilizarse entre sucursales de la misma
  organización sin permitir acceso entre organizaciones.
- Desactivar un proveedor no cambia ni revierte hechos de compra futuros.
- La base incluye `UNIQUE (id, organization_id)` para habilitar FKs compuestas
  same-organization de órdenes futuras.
- Esta decisión no autoriza crear órdenes, recepciones, movimientos
  `PURCHASE_RECEIPT`, costos, impuestos, pagos a proveedores ni devoluciones.

## Decisiones diferidas

- Convención de código comercial de PurchaseOrder.
- Estados, recepciones parciales, idempotencia y orden de locks de órdenes y
  recepciones.
- Protección definitiva de `InventoryItem.baseUnit` cuando existan dependencias
  de compras, además de las dependencias de Inventory actuales.
- Costeo, valoración, cuentas por pagar, pagos, devoluciones y contabilidad.
