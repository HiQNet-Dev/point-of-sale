# Convenciones de API

**Estado:** no hay endpoints de negocio aún; la API de foundation solo expone
`/actuator/health` y `/error` (deny by default). Las convenciones aplican desde
que se construya el módulo de API.

## Base path y formato

- Prefijo: **`/api/v1`** (ej. `POST /api/v1/sales` — futuro).
- JSON en requests y responses; claves en **`camelCase`**.
- Sin sufijos verbales en rutas (`/sales`, no `/getSales`).

## Semántica HTTP

| Método | Uso | Éxito |
| --- | --- | --- |
| GET | Lectura | 200 |
| POST | Creación | 201 |
| PUT/PATCH | Reemplazo / actualización parcial | 200 |
| DELETE | Baja | 204 |

Errores: 400 (validación/malformed), 401 (no autenticado), 403 (prohibido),
404 (recurso inexistente), 409 (conflicto), 422 (regla de negocio), 500 (interno).

## Contrato de error

Misma forma que `ApiError` del backend:

```json
{
  "timestamp": "2026-10-07T12:00:00Z",
  "status": 400,
  "code": "VALIDATION_ERROR",
  "message": "Datos de entrada inválidos",
  "path": "/api/v1/sales",
  "details": ["total: debe ser mayor o igual a 0"]
}
```

Códigos estables de ejemplo: `VALIDATION_ERROR`, `MALFORMED_REQUEST`,
`RESOURCE_NOT_FOUND`, `UNAUTHORIZED`, `FORBIDDEN`, `INTERNAL_ERROR`.
Nunca se exponen stack traces ni detalles internos.

## Validación

- **Siempre server-side**, incluso si el frontend ya validó.
- El frontend valida para usabilidad; el servidor valida para seguridad.

## Naming

- Rutas en **kebab-case**; recursos en **plural** (`/cash-registers`, `/sales`).
- IDs en la ruta para subrecursos: `/api/v1/branches/{branchId}/sales`.

## Paginación (futuro / no implementado)

- Listados: `?page=0&size=20&limit=...` (parámetros a fijar al construir el módulo).
- Operaciones de dinero de alto volumen podrían usar **cursor** (futuro).

## Idempotencia (futuro / no implementado)

- Operaciones monetarias (ventas, pagos) aceptarán cabecera
  **`Idempotency-Key`** para reintentos seguros. Aún no existe.

## OpenAPI

- **Todavía no hay OpenAPI** ni springdoc: el módulo de API no existe.
- Cuando se construya, springdoc generará la especificación en desarrollo.

## Seguridad de respuestas

- Nunca devolver datos sensibles: contraseñas, hashes, tokens, secretos,
  datos internos de infraestructura.
- Ver `docs/04-security/security-model.md`.
