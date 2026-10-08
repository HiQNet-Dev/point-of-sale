# Convenciones de API

**Estado:** existen los primeros endpoints de negocio
(Organization + Branch, `/api/v1`) y la autenticación (`auth`,
`/api/v1/auth`). La API de negocio está detrás de **deny-by-default** incluso
con un access JWT válido: abrirá solo cuando exista RBAC (módulo `users`).
Los tests legacy de Organization/Branch usan una configuración exclusiva de
`src/test` (`TestSecurityConfig`), nunca del artefacto productivo; los tests
de `auth` corren contra la configuración de seguridad **real**.
Detalle de contrato: `docs/02-database/organization-branch-model.md`,
`docs/04-security/authentication.md` y la especificación OpenAPI
(`/v3/api-docs`, swagger-ui activo solo en dev con `permit-opendoc=true`).

## Base path y formato

- Prefijo: **`/api/v1`**.
- JSON en requests y responses; claves en **`camelCase`**.
- Sin sufijos verbales en rutas (`/organizations`, no `/getOrganizations`).
- IDs en la ruta para subrecursos: `/organizations/{organizationId}/branches`.
- Recursos en **plural** (`/organizations`, `/branches`).

## Endpoints actuales

```text
POST  /api/v1/organizations
GET   /api/v1/organizations/{organizationId}
PATCH /api/v1/organizations/{organizationId}

POST  /api/v1/organizations/{organizationId}/branches
GET   /api/v1/organizations/{organizationId}/branches
GET   /api/v1/organizations/{organizationId}/branches/{branchId}
PATCH /api/v1/organizations/{organizationId}/branches/{branchId}

POST  /api/v1/auth/login        (público)
POST  /api/v1/auth/refresh      (público)
POST  /api/v1/auth/logout       (público; revoca la familia de la sesión presentada)
POST  /api/v1/auth/logout-all   (Bearer)
GET   /api/v1/auth/me           (Bearer)
```

No hay `DELETE`: la baja es `status = INACTIVE` (sin hard delete sobre datos
que luego serán financieros).

## Semántica HTTP

| Método | Uso | Éxito |
| --- | --- | --- |
| GET | Lectura | 200 |
| POST | Creación | 201 |
| PATCH | Actualización **parcial** | 200 |
| DELETE | No usado todavía | — |

Errores: 400 (validación/malformed), 401 (no autenticado), 403 (prohibido),
404 (recurso inexistente), 409 (conflicto: duplicado o modificación
concurrente), 500 (interno).

## PATCH (parcial, no PUT disfrazado)

- Solo se actualizan los campos presentes (no nulos) en el body.
- `code` es **inmutable** tras la creación, tanto en `organizations` como en
  `branches` (no se expone en PATCH): evita romper futuras referencias y URLs
  lógicas.
- `status` permite `ACTIVE`/`INACTIVE` (activar/desactivar).
- `branch.timezone` tiene semántica tri-estado explícita
  (`PatchField` en `shared.api`): omitir la clave = sin cambios;
  `{"timezone": "America/Mexico_City"}` = establecer override;
  `{"timezone": null}` = eliminar el override y volver a heredar
  `Organization.timezone`.

## Contrato de error

Misma forma que `ApiError` del backend:

```json
{
  "timestamp": "2026-10-07T12:00:00Z",
  "status": 409,
  "code": "BRANCH_CODE_ALREADY_EXISTS",
  "message": "A branch with this code already exists in the organization",
  "path": "/api/v1/organizations/…/branches",
  "details": []
}
```

Códigos estables:

| Código | HTTP | Cuándo |
| --- | --- | --- |
| `VALIDATION_ERROR` | 400 | Bean Validation o parámetro mal formado |
| `MALFORMED_REQUEST` | 400 | Body ilegible |
| `INVALID_CODE` | 400 | Slug inválido (formato `^[a-z0-9]+(-[a-z0-9]+)*$`) |
| `INVALID_TIMEZONE` | 400 | No es timezone IANA |
| `INVALID_CURRENCY` | 400 | No es ISO 4217 |
| `INVALID_LOCALE` | 400 | No es BCP 47 |
| `INVALID_CREDENTIALS` | 401 | Login fallido (genérico: org, email o password) |
| `INVALID_REFRESH_TOKEN` | 401 | Refresh desconocido, consumido, revocado o de familia muerta |
| `AUTHENTICATION_REQUIRED` | 401 | Sin token de acceso |
| `TOKEN_INVALID` | 401 | Access rechazado (firma, issuer, audiencia, tipo, `sub`/`org_id`) o identidad no operativa (`/me`) |
| `TOKEN_EXPIRED` | 401 | Access expirado |
| `FORBIDDEN` | 403 | Sin permiso (incluye APIs de negocio hasta RBAC) |
| `ORGANIZATION_NOT_FOUND` | 404 | Organización inexistente |
| `BRANCH_NOT_FOUND` | 404 | Branch inexistente (o de otra organización) |
| `RESOURCE_NOT_FOUND` | 404 | Ruta sin mapping |
| `ORGANIZATION_CODE_ALREADY_EXISTS` | 409 | Code de organización duplicado (global) |
| `BRANCH_CODE_ALREADY_EXISTS` | 409 | Code de branch duplicado en la organización |
| `CONCURRENT_MODIFICATION` | 409 | Falló optimistic locking |
| `INTERNAL_ERROR` | 500 | Error inesperado |

Nunca se exponen stack traces, mensajes SQL ni detalles internos.

## Validación

- **Siempre server-side**, incluso si el frontend validó (usabilidad).
- Doble capa: Bean Validation en DTOs (`VALIDATION_ERROR`) + invariantes de
  dominio en entidad/servicio (`INVALID_TIMEZONE`, `INVALID_CURRENCY`,
  `INVALID_LOCALE`, `INVALID_CODE`, duplicados).
- Constraints reales en PostgreSQL: `NOT NULL`, `UNIQUE`, `FOREIGN KEY`,
  `CHECK` (status, formato de code, longitud de moneda). No se confía solo en
  la capa de aplicación.

## Aislamiento entre organizaciones (IDOR)

Toda operación sobre una branch exige `organization_id`: un
`GET /organizations/{A}/branches/{B1}` donde B1 pertenece a otra organización
responde **404 BRANCH_NOT_FOUND**, nunca 200. Patrón obligatorio para módulos
futuros scoped por organización.

## OpenAPI

- `springdoc-openapi` **3.1.1** (compatible con Spring Boot 4.1.x).
- Especificación: `/v3/api-docs`; UI: `/swagger-ui` (solo cuando
  `app.security.permit-opendoc=true`, activo únicamente en `dev`).
- En producción los endpoints de docs siguen **denegados** por
  deny-by-default.

## Otros

- **Paginación** (futuro): listados con `?page=0&size=20` a fijar al
  construirlos.
- **Idempotencia** (futuro): operaciones monetarias aceptarán
  `Idempotency-Key`. Aún no existe.
- **Seguridad de respuestas:** nunca devolver contraseñas, hashes, tokens,
  secretos ni datos internos de infraestructura.