# API de órdenes de compra de Purchasing

**Estado:** implementada en `v0.4.0-SNAPSHOT` (V23). Solo administra **borradores y
partidas**; la emisión y la cancelación llegan en un incremento posterior.

El contrato técnico se genera desde las anotaciones OpenAPI y está disponible en
`/v3/api-docs` y Swagger UI en desarrollo. Esta guía explica cómo consumir las
rutas sin sustituir la especificación generada.

## Alcance

Una orden de compra pertenece a una organización y apunta a una sucursal de
destino. En V23 solo existe el estado `DRAFT`: crear la orden, editar la cabecera y
gestionar partidas. No hay recepción de mercancía, ni movimientos de inventario,
ni costeo, ni cuentas por pagar.

## Autenticación y permisos

Todas las rutas requieren `Authorization: Bearer <accessToken>` y están anidadas
bajo organización y sucursal. Los permisos son **branch-scoped** (un rol
organization-wide o una membership activa de esa sucursal):

| Operación | Permiso |
| --- | --- |
| Listar o consultar órdenes | `purchasing.order.read` |
| Crear borrador, editar cabecera y gestionar partidas | `purchasing.order.write` |
| Emitir una orden (pendiente) | `purchasing.order.issue` |
| Cancelar una orden (pendiente) | `purchasing.order.cancel` |

La migración V23 agrega los cuatro permisos al catálogo, pero **no** los concede a
ningún rol existente; deben asignarse mediante la administración RBAC. Un bootstrap
seguro nuevo recibe el catálogo completo.

Solo las lecturas usan `hasBranchPermissionIgnoringBranchStatus`, para que el
historial siga consultable aunque la sucursal se desactive.

## Rutas

| Método | Ruta | Éxito |
| --- | --- | --- |
| `POST` | `/api/v1/organizations/{organizationId}/branches/{branchId}/purchase-orders` | `201` |
| `GET` | `/api/v1/organizations/{organizationId}/branches/{branchId}/purchase-orders` | `200` |
| `GET` | `.../purchase-orders/{purchaseOrderId}` | `200` |
| `PATCH` | `.../purchase-orders/{purchaseOrderId}` | `200` |
| `POST` | `.../purchase-orders/{purchaseOrderId}/lines` | `201` |
| `PATCH` | `.../purchase-orders/{purchaseOrderId}/lines/{purchaseOrderLineId}` | `200` |
| `POST` | `.../purchase-orders/{purchaseOrderId}/lines/{purchaseOrderLineId}/remove` | `200` |

La colección usa la envoltura estándar [PageResponse](pagination.md): `page` base
cero (default `0`), `size` default `20` y máximo `100`. El orden es fijo:
`createdAt DESC, id DESC`.

`GET` de la colección acepta los filtros opcionales `status` (`DRAFT`), `supplierId`
y `q` (búsqueda por `code`, sin distinguir mayúsculas).

## Crear un borrador

`code` y `supplierId` son obligatorios. El estado siempre es `DRAFT` y la moneda
se copia de la organización: **no** se envían en el cuerpo (si se envían, se
ignoran). El proveedor debe existir en la organización y estar `ACTIVE`; la
sucursal debe existir y estar `ACTIVE`.

```json
{
  "code": "po-2026-0001",
  "supplierId": "f4a0e638-d4ce-4f84-aa98-9d2a56e1dd3e"
}
```

La respuesta `201` devuelve `{ purchaseOrder: {...}, lines: [] }`, con
`status: "DRAFT"`, `currency: "MXN"`, los snapshots del proveedor, `version: 0` y
`createdByUserId`.

## Actualizar la cabecera

`PATCH` requiere `expectedVersion`. Solo se puede cambiar el proveedor
(`supplierId`) y solo mientras la orden está en `DRAFT`; el código, la moneda y el
estado son inmutables. Una actualización sin cambios reales es idempotente: no
incrementa la versión.

```json
{
  "expectedVersion": 0,
  "supplierId": "8c1f0a6b-1f2e-4d3c-9a4b-5c6d7e8f9012"
}
```

## Gestionar partidas

Agregar partida (`POST .../lines`) con `expectedVersion`, `inventoryItemId`,
`quantity` como **string decimal** estrictamente positivo (hasta 6 decimales) y
`unitCost` opcional como string decimal no negativo (hasta 4 decimales):

```json
{
  "expectedVersion": 0,
  "inventoryItemId": "b6d2a1f3-6a4e-4a2e-9a3f-2f1c8d7b6a50",
  "quantity": "25000.000000",
  "unitCost": "0.0180"
}
```

Reglas:
- El artículo debe existir en la organización y estar `ACTIVE`.
- Un artículo solo puede aparecer **una vez** por orden (`409
  PURCHASE_ORDER_LINE_DUPLICATE_ITEM`).
- La respuesta incluye los snapshots `itemCodeSnapshot`, `itemNameSnapshot` y
  `unitSnapshot` capturados en ese momento; un renombrado posterior del artículo no
  los altera.
- `lineNumber` lo asigna el servidor (1, 2, 3, …).

`PATCH .../lines/{lineId}` actualiza `quantity` y/o `unitCost`; un campo ausente no
se modifica. El costo unitario usa semántica tri-estado: omitir `unitCost` lo deja
igual, un `null` explícito lo limpia a `NULL` y un string decimal (`"0.0000"`
incluido) lo asigna. El artículo y sus snapshots son inmutables.

`POST .../lines/{lineId}/remove` elimina físicamente la partida, solo en `DRAFT`, y
requiere `expectedVersion`.

**Toda mutación de partida incrementa la versión de la cabecera** (el agregado se
sincroniza en la fila de cabecera): tras agregar una partida a un borrador nuevo,
`version` pasa de `0` a `1`.

## Concurrencia

`expectedVersion` es obligatorio en toda mutación de un recurso existente. Si no
coincide con la versión actual, la respuesta es `409 CONCURRENT_MODIFICATION` y el
cliente debe releer la orden antes de reintentar. Dos escrituras concurrentes sobre
la misma orden se serializan: solo una gana y la otra recibe `409`.

## Errores relevantes

Todas las respuestas de error usan [ApiError](errors.md).

| HTTP | Código | Escenario |
| --- | --- | --- |
| `400` | `VALIDATION_ERROR` | Datos, paginación o formato de código inválidos. |
| `400` | `INVALID_CODE` | El código no es un slug en minúsculas. |
| `400` | `INVALID_INVENTORY_QUANTITY` | Cantidad no es un decimal válido o excede `NUMERIC(19,6)`. |
| `400` | `MALFORMED_REQUEST` | Cuerpo JSON o enum inválidos. |
| `401` | `AUTHENTICATION_REQUIRED` / `TOKEN_INVALID` / `TOKEN_EXPIRED` | Falta o no es válido el Bearer token. |
| `403` | `FORBIDDEN` | El token no tiene el permiso requerido en la sucursal. |
| `404` | `ORGANIZATION_NOT_FOUND` / `BRANCH_NOT_FOUND` | La organización o la sucursal no existen en el ámbito de la ruta. |
| `404` | `SUPPLIER_NOT_FOUND` / `INVENTORY_ITEM_NOT_FOUND` | El proveedor o el artículo no pertenecen a la organización. |
| `404` | `PURCHASE_ORDER_NOT_FOUND` | La orden no existe, es de otra organización o de otra sucursal. |
| `404` | `PURCHASE_ORDER_LINE_NOT_FOUND` | La partida no existe o no pertenece a la orden. |
| `409` | `PURCHASE_ORDER_CODE_ALREADY_EXISTS` | El código ya existe en la organización (incluso en creación concurrente). |
| `409` | `PURCHASE_ORDER_LINE_DUPLICATE_ITEM` | Ya existe una partida de ese artículo en la orden. |
| `409` | `PURCHASE_ORDER_NOT_DRAFT` | La orden no está en `DRAFT` y no admite edición. |
| `409` | `SUPPLIER_INACTIVE` / `INVENTORY_ITEM_INACTIVE` / `BRANCH_INACTIVE` | El recurso referenciado está inactivo. |
| `409` | `CONCURRENT_MODIFICATION` | `expectedVersion` no coincide con la versión actual. |

No interprete un `404` como autorización: con permisos solo en otra organización la
autorización se deniega primero con `403`, conforme al modelo RBAC global. Una
orden de otra sucursal de la **misma** organización responde `404`, porque la
sucursal forma parte del ámbito del recurso.
