# API de proveedores de Purchasing

**Estado:** implementada en `v0.4.0` (V22).

El contrato técnico se genera desde las anotaciones OpenAPI y está disponible en
`/v3/api-docs` y Swagger UI en desarrollo. Esta guía explica cómo consumir las
rutas de proveedores sin sustituir la especificación generada.

## Alcance

Un proveedor es un recurso administrativo propiedad de la Organization. No hay
`DELETE`: se usa `status: "INACTIVE"` para conservar futuras referencias de
compras. Este incremento no incluye órdenes, recepciones, pagos, costos ni
movimientos de inventario.

## Autenticación y permisos

Todas las rutas requieren `Authorization: Bearer <accessToken>`. Obtenga el
token mediante `POST /api/v1/auth/login` y péguelo en **Authorize** de Swagger;
la receta completa está en [authentication.md](authentication.md).

| Operación | Permiso organization-scoped |
| --- | --- |
| Listar o consultar proveedor | `purchasing.supplier.read` |
| Crear o actualizar proveedor | `purchasing.supplier.write` |

Los permisos se evalúan en PostgreSQL por petición. La migración V22 los agrega
al catálogo, pero no los concede a roles existentes; debe asignarlos mediante
la administración RBAC. Un bootstrap seguro nuevo recibe el catálogo completo.

## Rutas

| Método | Ruta | Éxito |
| --- | --- | --- |
| `POST` | `/api/v1/organizations/{organizationId}/suppliers` | `201` |
| `GET` | `/api/v1/organizations/{organizationId}/suppliers` | `200` |
| `GET` | `/api/v1/organizations/{organizationId}/suppliers/{supplierId}` | `200` |
| `PATCH` | `/api/v1/organizations/{organizationId}/suppliers/{supplierId}` | `200` |

Las colecciones usan la envoltura estándar
[PageResponse](pagination.md): `page` base cero (default `0`), `size` default
`20` y máximo `100`. El orden no es configurable: `name ASC, id ASC`.

`GET` acepta los filtros opcionales:

- `status`: `ACTIVE` o `INACTIVE`.
- `q`: busca sin distinción de mayúsculas en `code`, `name`, `legalName`,
  `taxId` y `contactName`.

## Crear proveedor

`code` y `name` son obligatorios. El código es un slug en minúsculas de hasta 64
caracteres, único en la organización e inmutable. Los datos de contacto son
opcionales; el email se normaliza a minúsculas.

```http
POST /api/v1/organizations/{organizationId}/suppliers
Authorization: Bearer <accessToken>
Content-Type: application/json
```

```json
{
  "code": "distribuidora-centro",
  "name": "Distribuidora Centro",
  "legalName": "Distribuidora Centro, S.A. de C.V.",
  "taxId": "DCE010203AB4",
  "contactName": "Ana López",
  "email": "ventas@distribuidora-centro.example",
  "phone": "+52 55 5555 0101",
  "address": "Av. Central 100, Ciudad de México"
}
```

La respuesta `201` devuelve todos los campos, incluidos `id`, `organizationId`,
`status: "ACTIVE"`, `version: 0`, `createdAt` y `updatedAt`.

## Actualizar proveedor

`PATCH` requiere `expectedVersion` no negativo. El backend rechaza una versión
obsoleta con `409 CONCURRENT_MODIFICATION`. `code` no aparece en el payload y
no puede cambiarse. Los campos omitidos o enviados como `null` no se modifican;
una cadena vacía elimina un dato de texto opcional; los demás textos opcionales
pueden limpiarse también con una cadena formada solo por espacios.

```json
{
  "expectedVersion": 0,
  "contactName": "Ana López",
  "email": "ventas-norte@distribuidora-centro.example",
  "status": "ACTIVE"
}
```

Para retirar el proveedor, se actualiza solamente:

```json
{
  "expectedVersion": 1,
  "status": "INACTIVE"
}
```

## Errores relevantes

Todas las respuestas de error usan
[ApiError](errors.md), incluido `timestamp`, `status`, `code`, `message`,
`path` y `details` cuando corresponde.

| HTTP | Código | Escenario |
| --- | --- | --- |
| `400` | `VALIDATION_ERROR` / `INVALID_CODE` | Datos, paginación, email o formato de código inválidos. |
| `401` | `AUTHENTICATION_REQUIRED` | Falta o no es válido el Bearer token. |
| `403` | `FORBIDDEN` | El token no tiene el permiso requerido dentro de la organización. |
| `404` | `ORGANIZATION_NOT_FOUND` / `SUPPLIER_NOT_FOUND` | La organización no existe o el proveedor no pertenece a la organización de la ruta. |
| `409` | `SUPPLIER_CODE_ALREADY_EXISTS` | El código ya existe dentro de esa organización, incluso bajo creación concurrente. |
| `409` | `CONCURRENT_MODIFICATION` | `expectedVersion` no coincide con la versión actual. |

No interprete un `404` como autorización: con permisos solo en otra
organización, la autorización se deniega primero con `403`, conforme al modelo
RBAC global.
