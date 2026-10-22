# Modelo de proveedores de Purchasing — V22

**Estado:** implementado en `V22__create_purchasing_supplier_foundation.sql`.

## Ownership y ciclo de vida

`suppliers` es una tabla de datos maestros propiedad de una Organization. No
contiene `branch_id`: un mismo proveedor puede ser seleccionado por futuras
órdenes de distintas sucursales de la organización. Toda consulta de aplicación
incluye `organization_id`; un UUID de proveedor no se resuelve globalmente.

No existe eliminación física. `status` acepta `ACTIVE` e `INACTIVE`; la baja
lógica preserva referencias históricas que crearán los incrementos posteriores.

## Columnas e integridad

| Columna | Regla |
| --- | --- |
| `id` | UUID, PK generado por backend. |
| `organization_id` | FK obligatoria a `organizations`. |
| `code` | `VARCHAR(64)`, slug en minúsculas, inmutable y único por organización. |
| `name` | `VARCHAR(120)`, obligatorio y no vacío. |
| `legal_name`, `tax_id`, `contact_name` | Textos opcionales no vacíos cuando se informan. |
| `email`, `phone`, `address` | Datos de contacto opcionales; el email se valida y normaliza en backend. |
| `status` | `ACTIVE` o `INACTIVE`, default `ACTIVE`. |
| `version` | Control optimista; `PATCH` requiere `expectedVersion`. |
| `created_at`, `updated_at` | Instantes UTC gestionados por la entidad. |

La migración define `UNIQUE (organization_id, code)` y también
`UNIQUE (id, organization_id)`. La segunda es una superclave necesaria para que
futuras tablas de Purchasing puedan imponer FKs compuestas que impidan cruzar
proveedores entre organizaciones.

El índice `(organization_id, name, id)` sostiene el listado ordenado de forma
determinista por `name ASC, id ASC`.

## Concurrencia y auditoría

La creación realiza una comprobación previa por usabilidad, pero una carrera se
resuelve definitivamente por el constraint único de PostgreSQL y retorna
`409 SUPPLIER_CODE_ALREADY_EXISTS`. Las actualizaciones bloquean la fila scoped
y validan `expectedVersion`; una versión obsoleta retorna
`409 CONCURRENT_MODIFICATION`.

Cada creación o cambio real escribe un evento append-only en `audit_events`
dentro de la misma transacción. Los eventos son `supplier.created` y
`supplier.updated`; su metadata no contiene datos de contacto, dirección ni
identificador fiscal.

## Fuera de alcance de V22

V22 no crea Purchase Orders, Purchase Receipts, cuentas por pagar, pagos,
movimientos de stock, costos, impuestos, devoluciones ni contabilidad. Tampoco
altera Inventory ni el ledger inmutable existente.
