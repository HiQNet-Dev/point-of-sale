# Convenciones de API

**Estado:** existen los primeros endpoints de negocio
(Organization + Branch, `/api/v1`), la autenticación (`auth`, `/api/v1/auth`),
la **administración de usuarios** y la **administración de roles/permisos**
(`rbac`), todas scoped por organización. Desde RBAC la API administrativa está
**autorizada por permisos** con PostgreSQL como autoridad (JWT = identidad;
método security con `@PreAuthorize`); `POST /organizations` y todo lo no
definido siguen **deny-by-default**. Los tests de negocio de los primeros
slices corren contra la cadena de seguridad **real** con JWTs reales
(`AbstractRbacIntegrationTest`); solo el endpoint `POST /organizations`
(que no tiene permiso) conserva un test bajo la configuración exclusiva de
`src/test`.
Detalle de contrato: `docs/02-database/organization-branch-model.md`,
`docs/04-security/rbac.md`, `docs/04-security/authentication.md` y la
especificación OpenAPI
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

POST  /api/v1/organizations/{organizationId}/users
GET   /api/v1/organizations/{organizationId}/users        (paginado, ?page&size&status)
GET   /api/v1/organizations/{organizationId}/users/{userId}
PATCH /api/v1/organizations/{organizationId}/users/{userId}
GET   /api/v1/organizations/{organizationId}/users/{userId}/roles
PUT   /api/v1/organizations/{organizationId}/users/{userId}/roles

GET   /api/v1/organizations/{organizationId}/permissions (catálogo, code ASC)

POST  /api/v1/organizations/{organizationId}/roles
GET   /api/v1/organizations/{organizationId}/roles        (paginado, ?page&size&status)
GET   /api/v1/organizations/{organizationId}/roles/{roleId}
PATCH /api/v1/organizations/{organizationId}/roles/{roleId}
GET   /api/v1/organizations/{organizationId}/roles/{roleId}/permissions
PUT   /api/v1/organizations/{organizationId}/roles/{roleId}/permissions

POST  /api/v1/organizations/{organizationId}/branches/{branchId}/memberships
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/memberships     (paginado, ?page&size&status)
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/memberships/{userId}
PATCH /api/v1/organizations/{organizationId}/branches/{branchId}/memberships/{userId}
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/memberships/{userId}/roles
PUT   /api/v1/organizations/{organizationId}/branches/{branchId}/memberships/{userId}/roles

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

## Branch Memberships (scopes de autorización)

La gestión de memberships se autoriza SIEMPRE con permisos **organization-wide**
(`branch.membership.read/.create/.update/.roles.manage`), nunca branch-scoped
(anti privilege-escalation). En cambio `GET/PATCH /branches/{branchId}` usan
`hasBranchPermission(…#branchId…)`: vale un permiso organization-wide **o** una
membership ACTIVE de ESA branch (Branch ACTIVE incluido en la query).

- **POST** crea la membership (`userId` en el body). Exige Organization, Branch y
  User existentes, de la misma organización y `ACTIVE` (409 `USER_INACTIVE` /
  `BRANCH_INACTIVE` / `ORGANIZATION_INACTIVE`; duplicado → 409
  `BRANCH_MEMBERSHIP_ALREADY_EXISTS`).
- **GET list**: `PageResponse`; `?page`(0) `?size`(20, máx 100); orden
  `createdAt ASC, id ASC`; filtro `?status=ACTIVE|INACTIVE`.
- **GET /{userId}** y **PATCH /{userId}**: la membership se direcciona por el
  `userId` (máx. una membership por user+branch). PATCH exige `expectedVersion`;
  sin DELETE (baja = `status INACTIVE`).
- **PUT /{userId}/roles**: replace atómico del set completo; duplicados → 400
  `VALIDATION_ERROR`; rol inexistente o de otra org → 404 `ROLE_NOT_FOUND` con
  rollback total; `[]` limpia.

## Optimistic concurrency HTTP (expectedVersion)

Desde RBAC, todo PATCH administrativo (Organization, Branch, User, Role,
**Membership**) exige:

```json
{ "expectedVersion": 3, "name": "Nuevo nombre" }
```

- `expectedVersion` es **requerido** y `>= 0` (ausente/negativo → `400
  VALIDATION_ERROR`); declara la versión que el cliente leyó, jamás fija la
  nueva.
- Antes de mutar: `entity.version == expectedVersion`; si no → `409
  CONCURRENT_MODIFICATION`. `@Version` sigue protegiendo carreras reales.
- Sin ETag / If-Match por ahora (contrato explícito `expectedVersion`).

## POST /organizations permanece cerrado

`POST /api/v1/organizations` NO tiene permiso (`organization.create` no existe)
y permanece deny-by-default incluso con un JWT con todos los permisos actuales
(403). Crear organizaciones será del secure bootstrap / futuro Control Plane.

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
| `ORGANIZATION_INACTIVE` | 409 | No se crean usuarios en una organización inactiva (el HTTP real responde 403 en la frontera de autorización) |
| `BRANCH_NOT_FOUND` | 404 | Branch inexistente (o de otra organización) |
| `USER_NOT_FOUND` | 404 | Usuario inexistente (o de otra organización) |
| `USER_EMAIL_ALREADY_EXISTS` | 409 | Email duplicado dentro de la organización |
| `WEAK_PASSWORD` | 400 | Password fuera de la política (12–128 caracteres) |
| `ROLE_NOT_FOUND` | 404 | Rol inexistente (o de otra organización) |
| `ROLE_CODE_ALREADY_EXISTS` | 409 | Code de rol duplicado dentro de la organización |
| `PERMISSION_NOT_FOUND` | 404 | Permiso inexistente en el catálogo |
| `BRANCH_MEMBERSHIP_NOT_FOUND` | 404 | Membership inexistente (usuario sin membership en esa branch) |
| `BRANCH_MEMBERSHIP_ALREADY_EXISTS` | 409 | Membership duplicada (org+branch+user), incluida la carrera (constraint) |
| `BRANCH_INACTIVE` | 409 | No se crean memberships en una branch inactiva |
| `USER_INACTIVE` | 409 | No se crean memberships para un usuario inactivo |
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

Toda operación sobre un recurso scoped exige `organization_id`: un
`GET /organizations/{A}/branches/{B1}`, `GET /organizations/{A}/users/{U1}` o
`GET /organizations/{A}/roles/{R1}` donde el recurso pertenece a otra
organización responde **404** (`BRANCH_NOT_FOUND` / `USER_NOT_FOUND` /
`ROLE_NOT_FOUND`), nunca 200. Distinción de capas con RBAC:

- **Frontera de autorización** (sin permiso, identidad/org no operativa, otra
  organización sin permiso): **403 `FORBIDDEN`**.
- **Aislamiento de recurso** (permiso válido en la org de la ruta, id ajeno):
  **404**.
- Sin token: **401 `AUTHENTICATION_REQUIRED`**.

## Paginación

Convención estable desde la administración de usuarios (`shared.api.PageResponse`):

```json
{ "items": [], "page": 0, "size": 20, "totalElements": 0, "totalPages": 0 }
```

- Parámetros `?page` (default 0) y `?size` (default 20, **máximo 100**).
- Orden determinista en servidor (no se aceptan campos de orden del cliente):
  en `users`, `email ASC, id ASC`.
- Nunca se serializa `Page`/`Pageable` de Spring Data directamente.

## OpenAPI

- `springdoc-openapi` **3.1.1** (compatible con Spring Boot 4.1.x).
- Especificación: `/v3/api-docs`; UI: `/swagger-ui` (solo cuando
  `app.security.permit-opendoc=true`, activo únicamente en `dev`).
- En producción los endpoints de docs siguen **denegados** por
  deny-by-default.

## Otros

- **Optimistic concurrency HTTP**: resuelto como contrato `expectedVersion` en
  Organization + Branch + User + Role + Membership (sin ETag/If-Match por ahora;
  revisar antes de abrir las APIs administrativas en producción).
- **SCOPES**: `hasOrganizationPermission` (organization-wide) vs
  `hasBranchPermission` (org-wide UNION membership branch-scoped) —
  documentado en `docs/04-security/rbac.md`.
- **Idempotencia** (futuro): operaciones monetarias aceptarán
  `Idempotency-Key`. Aún no existe.
- **Seguridad de respuestas:** nunca devolver contraseñas, hashes, tokens,
  secretos ni datos internos de infraestructura.