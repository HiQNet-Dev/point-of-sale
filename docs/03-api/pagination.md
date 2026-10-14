# Paginación

Contrato común de los endpoints de colección que paginan (clase
`com.hiqnet.pos.shared.api.PageResponse`). Referencia canónica:
`api-guidelines.md` (§ Paginación).

## Envoltura `PageResponse`

```json
{
  "items": [],
  "page": 0,
  "size": 20,
  "totalElements": 0,
  "totalPages": 0
}
```

| Campo | Significado |
| --- | --- |
| `items` | Página actual (nunca se serializa `Page`/`Pageable` de Spring Data directamente). |
| `page` | Índice de página, base **0**. |
| `size` | Tamaño solicitado. |
| `totalElements` | Total de elementos que cumplen el filtro. |
| `totalPages` | Total de páginas. |

## Parámetros

| Parámetro | Default | Límite |
| --- | --- | --- |
| `page` | `0` | `>= 0`; fuera de rango → `400 VALIDATION_ERROR`. |
| `size` | `20` | máximo **100**. |

El orden es **determinista y fijado por el servidor**; no se aceptan campos de
orden enviados por el cliente. Los filtros disponibles (`status`, `q`,
`categoryId`, …) los documenta cada endpoint en Swagger.

## Excepción: `GET /branches` no pagina

`GET /api/v1/organizations/{organizationId}/branches` devuelve la colección
completa sin `PageResponse` (las sucursales de una organización son pocas por
naturaleza). El caso “mis sucursales” sí está paginado en
`GET /api/v1/auth/me/branches`.

## Ejemplo

```text
GET /api/v1/organizations/{organizationId}/products?page=0&size=20&status=ACTIVE&q=cafe
```

```json
{
  "items": [ { "id": "…", "sku": "CAFE-01", "name": "Café americano", "unitPrice": 35.0, "status": "ACTIVE" } ],
  "page": 0,
  "size": 20,
  "totalElements": 1,
  "totalPages": 1
}
```

> Algunos endpoints usan defaults distintos (p. ej. `audit-events` con
> `size` por defecto 50). Consulte el parámetro en Swagger para el endpoint
> concreto.
