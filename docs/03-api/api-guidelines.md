# Convenciones de API

**Estado:** existen los primeros endpoints de negocio
(Organization + Branch, `/api/v1`), la autenticación (`auth`, `/api/v1/auth`),
la **administración de usuarios**, la **administración de roles/permisos**
(`rbac`) y el **catálogo** (`catalog`: categorías, productos y **modifiers**) y
las **cajas** (`cashregister`),
todas scoped por organización. Desde RBAC la API administrativa está
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

POST  /api/v1/organizations/{organizationId}/categories
GET   /api/v1/organizations/{organizationId}/categories              (paginado, ?page&size&status)
GET   /api/v1/organizations/{organizationId}/categories/{categoryId}
PATCH /api/v1/organizations/{organizationId}/categories/{categoryId}

POST  /api/v1/organizations/{organizationId}/products
GET   /api/v1/organizations/{organizationId}/products                (paginado, ?page&size&status&categoryId&q)
GET   /api/v1/organizations/{organizationId}/products/{productId}
PATCH /api/v1/organizations/{organizationId}/products/{productId}

POST  /api/v1/organizations/{organizationId}/modifier-groups
GET   /api/v1/organizations/{organizationId}/modifier-groups          (paginado, ?page&size&status&q)
GET   /api/v1/organizations/{organizationId}/modifier-groups/{groupId}
PATCH /api/v1/organizations/{organizationId}/modifier-groups/{groupId}

POST  /api/v1/organizations/{organizationId}/modifier-groups/{groupId}/options
GET   /api/v1/organizations/{organizationId}/modifier-groups/{groupId}/options          (paginado, ?page&size&status)
GET   /api/v1/organizations/{organizationId}/modifier-groups/{groupId}/options/{optionId}
PATCH /api/v1/organizations/{organizationId}/modifier-groups/{groupId}/options/{optionId}

GET   /api/v1/organizations/{organizationId}/products/{productId}/modifier-groups   (product.read)
PUT   /api/v1/organizations/{organizationId}/products/{productId}/modifier-groups   (product.modifiers.manage)

POST  /api/v1/organizations/{organizationId}/branches/{branchId}/memberships
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/memberships     (paginado, ?page&size&status)
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/memberships/{userId}
PATCH /api/v1/organizations/{organizationId}/branches/{branchId}/memberships/{userId}
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/memberships/{userId}/roles
PUT   /api/v1/organizations/{organizationId}/branches/{branchId}/memberships/{userId}/roles

POST  /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers          (paginado, ?page&size&status&q)
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}
PATCH /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}

POST  /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts   (paginado, ?page&size&status)
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}
POST  /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/close

POST  /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales       (?page&size&status)
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales/{saleId}
POST  /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales/{saleId}/checkout
GET   /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales/{saleId}/payments
POST  /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales/{saleId}/items
PATCH /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales/{saleId}/items/{saleItemId}
POST  /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales/{saleId}/items/{saleItemId}/remove
POST  /api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales/{saleId}/cancel

GET   /api/v1/organizations/{organizationId}/audit-events  (audit.read; SOLO lectura)

POST  /api/v1/bootstrap   (ONE-SHOT provisioning; X-Bootstrap-Token; NO Bearer)

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
- `product.categoryId` tiene la misma semántica tri-estado, con `UuidPatchField`
  en `shared.api`: omitir = sin cambios; `{"categoryId": null}` = desasignar la
  categoría; `{"categoryId": "<uuid>"}` = asignar (misma organización y
  `ACTIVE`).
- `category.code` y `product.sku` son **inmutables** (no se aceptan en PATCH);
  una `sku` presente en el body se ignora.

## Audit log (solo lectura)

```text
GET /api/v1/organizations/{organizationId}/audit-events   (audit.read, organization scope)
```

- **Append-only**: el historial NO tiene `POST/PATCH/PUT/DELETE` — esos
  métodos no se implementan y caen en `denyAll()` (403).
- Paginación `PageResponse` con `?page`(0) `?size`(50, máx 100); orden fijo
  `occurredAt DESC, id DESC` (sin sort arbitrario).
- Filtros opcionales: `branchId`, `actorUserId`, `action`, `entityType`,
  `entityId`, `from`, `to`. Tiempo: `from` inclusivo / `to` exclusivo
  (`[from, to)`); `from > to` → `400 VALIDATION_ERROR`.
- `branchId`/`actorUserId` de otra organización → `404 BRANCH_NOT_FOUND` /
  `404 USER_NOT_FOUND` (aislamiento; nunca un filtrado silencioso a cero).
- `audit.read` SIEMPRE organization scope: un rol branch-scoped con `audit.read`
  → 403 (anti privilege-escalation).
- Detalle: `docs/04-security/audit.md`.

## Contrato de error
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
**Membership**, Category, Product, **ModifierGroup**, **ModifierOption**,
**CashRegister**) y el
`PUT` de reemplazo producto↔modifier-group exige:

```json
{ "expectedVersion": 3, "name": "Nuevo nombre" }
```

- `expectedVersion` es **requerido** y `>= 0` (ausente/negativo → `400
  VALIDATION_ERROR`); declara la versión que el cliente leyó, jamás fija la
  nueva.
- Antes de mutar: `entity.version == expectedVersion`; si no → `409
  CONCURRENT_MODIFICATION`. `@Version` sigue protegiendo carreras reales.
- Sin ETag / If-Match por ahora (contrato explícito `expectedVersion`).

## Secure Admin Bootstrap (one-shot provisioning)

`POST /api/v1/bootstrap` NO es una API administrativa regular: no lleva Bearer
ni exige permiso; se autoriza con el secret de deployment
(`X-Bootstrap-Token`, Base64 ≥ 32 bytes, validado en la cadena de seguridad ANTES
de leer el body, comparación constant-time, `401 BOOTSTRAP_INVALID_TOKEN`
genérico si falta/inválido). Sólo existe mientras `BOOTSTRAP_SECRET` está
configurado; si no, la ruta cae en deny-by-default.

- Crea Organization + User administrador + Role `administrator` (rol NORMAL con
  TODOS los permissions del catálogo leídos de PostgreSQL) + asignación
  organization-wide; UNA transacción, audit SYSTEM, estado singleton
  (`installation_bootstrap`, `V8`).
- `201` devuelve solo ids (nunca tokens/passwords). Login posterior:
  `POST /api/v1/auth/login`.
- `409 BOOTSTRAP_ALREADY_COMPLETED` (ya provisionado, con token válido) / `409
  BOOTSTRAP_NOT_ALLOWED` (organizations ya existen).
- Detalle: `docs/04-security/secure-bootstrap.md`, ADR-0006.

## Catalog (categorías y productos)

Catálogo organization-scoped (`category.*` / `product.*`; `hasPermission`, no
`hasBranchPermission`). Sin DELETE: `PUT`/`DELETE` no se implementan y caen en
`denyAll()`. Detalle de modelo: `docs/02-database/catalog-model.md`.

- **POST /categories**: crea (`code` slug único por org, inmutable; `name`
  trim). Duplicado → 409 `CATEGORY_CODE_ALREADY_EXISTS`; slug inválido → 400
  `VALIDATION_ERROR`.
- **GET /categories**: `PageResponse` (`?page`(0) `?size`(20, máx 100)
  `?status`); orden `name ASC, id ASC`.
- **PATCH /categories/{categoryId}**: `name` y/o `status`; `expectedVersion`
  requerido; `code` inmutable.
- **POST /products**: `sku` (canónico MAYÚSCULAS, inmutable, único por org),
  `barcode` opcional (único por org cuando presente, texto con ceros a la
  izquierda), `name`, `description` opcional, `categoryId` opcional (misma org
  y ACTIVE) y `unitPrice` (`>= 0`, `NUMERIC(19,4)`; moneda heredada). Duplicados
  → 409 `PRODUCT_SKU_ALREADY_EXISTS` / `PRODUCT_BARCODE_ALREADY_EXISTS`;
  categoría inactiva → 409 `CATEGORY_INACTIVE`; categoría ajena → 404
  `CATEGORY_NOT_FOUND`.
- **GET /products**: `PageResponse` (filtros `?status&categoryId&q`; `q` busca
  en `name`/`sku`/`barcode`); orden `name ASC, id ASC`. Un `categoryId` de otra
  organización → 404 `CATEGORY_NOT_FOUND` (nunca página vacía).
- **PATCH /products/{productId}**: campos opcionales + `expectedVersion`;
  `categoryId` tri-estado (omitir/null/valor); `sku` inmutable.

## Product Modifiers (grupos, opciones y asignación)

Modifiers organization-scoped (`modifier.group.*` / `modifier.option.*`; la
asignación producto↔grupo usa `product.read` y `product.modifiers.manage`).
`hasPermission`, no `hasBranchPermission`. Sin DELETE: `PUT`/`DELETE` no se
implementan y caen en `denyAll()`. Detalle de modelo:
`docs/02-database/product-modifiers-model.md`.

- **POST /modifier-groups**: crea (`code` slug único por org, inmutable; `name`;
  `minSelections` ≥0 / `maxSelections` ≥1 opcionales, default 0/1, con
  `minSelections ≤ maxSelections`). Duplicado → 409
  `MODIFIER_GROUP_CODE_ALREADY_EXISTS`; bounds inválidos o slug inválido → 400
  `VALIDATION_ERROR`. La respuesta incluye los derivados `required`,
  `singleChoice`, `multipleChoice`.
- **GET /modifier-groups**: `PageResponse` (`?page`(0) `?size`(20, máx 100)
  `?status` `?q` — `q` busca en `name`/`code`); orden `name ASC, id ASC`.
- **GET /modifier-groups/{groupId}** y **PATCH**: `expectedVersion` requerido;
  `code` inmutable.
- **POST /modifier-groups/{groupId}/options**: crea dentro del grupo
  (`code` slug inmutable único **dentro del grupo**; `name`; `priceDelta`
  `NUMERIC(19,4)` **con signo**; `displayOrder` ≥0). Grupo inexistente o de otra
  org → 404 `MODIFIER_GROUP_NOT_FOUND`; duplicado en el grupo → 409
  `MODIFIER_OPTION_CODE_ALREADY_EXISTS`.
- **GET /modifier-groups/{groupId}/options**: `PageResponse` (`?status`); orden
  `displayOrder ASC, id ASC`.
- **GET/PATCH /modifier-groups/{groupId}/options/{optionId}**: `expectedVersion`
  requerido; `code` inmutable.
- **GET /products/{productId}/modifier-groups**: lista ordenada (`displayOrder`)
  de grupos asignados; requiere `product.read`.
- **PUT /products/{productId}/modifier-groups**: **reemplazo atómico** completo
  del set ordenado (`modifierGroupIds`; la posición es el `displayOrder`). `[]`
  limpia; duplicados → 400 `VALIDATION_ERROR`; grupo inexistente o de otra org →
  404 `MODIFIER_GROUP_NOT_FOUND`. `expectedVersion` del PRODUCTO es la frontera
  de concurrencia (mismatch → 409 `CONCURRENT_MODIFICATION`); una lista idéntica
  es **no-op** (sin mutación, sin bump de versión, sin auditoría). Requiere
  `product.modifiers.manage`.

## Cash Registers (cajas)

Cajas **branch-owned**, anidadas bajo una branch. Autorización branch-scoped
(`cash-register.read` / `cash-register.create` / `cash-register.update` con
`hasBranchPermission`: org-wide **o** membership ACTIVE de esa branch), nunca
promovida a org-scope. Sin DELETE: `PUT`/`DELETE` no se implementan y caen en
`denyAll()`. Detalle de modelo: `docs/02-database/cash-register-model.md`.

- **POST /branches/{branchId}/cash-registers**: crea (`code` slug único **por
  branch**, inmutable; `name` trim no-blank, máx 120). Branch inexistente o de
  otra org → 404 `BRANCH_NOT_FOUND`; `code` duplicado en la branch → 409
  `CASH_REGISTER_CODE_ALREADY_EXISTS`; slug inválido → 400 `VALIDATION_ERROR`.
  El `status` NO se fija en el create (siempre `ACTIVE`); el `organization_id`
  se deriva de la branch.
- **GET /branches/{branchId}/cash-registers**: `PageResponse` (`?page`(0)
  `?size`(20, máx 100) `?status` `?q` — `q` busca en `name`/`code`); orden
  `name ASC, id ASC`.
- **GET/PATCH /branches/{branchId}/cash-registers/{cashRegisterId}**:
  `expectedVersion` requerido; `code` inmutable; `name` y/o `status`. Caja de
  otra branch/org → 404 `CASH_REGISTER_NOT_FOUND`; mismatch de versión → 409
  `CONCURRENT_MODIFICATION`; `PATCH` sin cambios efectivos es **no-op** (sin
  mutación, sin bump de versión, sin auditoría).

## Cash Shifts (turnos de caja)

Turnos **register-owned**, anidados bajo una cash register de una branch.
Autorización branch-scoped (`cash-shift.read` / `cash-shift.open` /
`cash-shift.close` con `hasBranchPermission`: org-wide **o** membership ACTIVE de
esa branch). Sin DELETE/PUT/PATCH: caen en `denyAll()`. Cerrar exige
`cash-shift.close` (no `cash-shift.open`). Detalle de modelo:
`docs/02-database/cash-shift-model.md`.

- **POST /cash-registers/{cashRegisterId}/cash-shifts** (open): body
  `{openingAmount}`; la caja (y su branch) deben existir y estar `ACTIVE`; los
  actores y timestamps los fija el servidor. Caja/branch de otra org o
  inexistente → 404; caja `INACTIVE` → 409 `CASH_REGISTER_INACTIVE`; branch
  `INACTIVE` → 409 `BRANCH_INACTIVE`; ya existe un turno `OPEN` → 409
  `CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT`; `openingAmount` negativo o con más de 4
  decimales → 400 `VALIDATION_ERROR`.
- **GET /cash-registers/{cashRegisterId}/cash-shifts**: `PageResponse` (`?page`(0)
  `?size`(20, máx 100) `?status` — `OPEN`/`CLOSED`); orden `openedAt DESC, id
  DESC`.
- **GET /cash-shifts/{cashShiftId}**: turno scoped por
  org+branch+caja+turno (`CASH_SHIFT_NOT_FOUND` si no coincide).
- **POST /cash-shifts/{cashShiftId}/close**: body
  `{expectedVersion, declaredCashAmount}`; cierra un turno `OPEN`. Ya cerrado →
  409 `CASH_SHIFT_ALREADY_CLOSED`; mismatch de versión → 409
  `CONCURRENT_MODIFICATION`; `declaredCashAmount` negativo o con más de 4
  decimales → 400 `VALIDATION_ERROR`. El cierre funciona aunque la caja/branch se
  hayan vuelto `INACTIVE`.

## POST /organizations permanece cerrado

`POST /api/v1/organizations` NO tiene permiso (`organization.create` no existe)
y permanece deny-by-default incluso con un JWT con todos los permisos actuales
(403). La creación de la primera organización vive en
`POST /api/v1/bootstrap` (provisioning), no en un permiso de negocio.

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
| `BOOTSTRAP_INVALID_TOKEN` | 401 | Bootstrap: token ausente/inválido/Base64 malformado, o bootstrap deshabilitado (siempre genérico) |
| `BOOTSTRAP_ALREADY_COMPLETED` | 409 | Bootstrap: instalación ya provisionada |
| `BOOTSTRAP_NOT_ALLOWED` | 409 | Bootstrap: instalación no pristina (organizations existentes) |
| (`audit.read`) | 403 | Permiso organization-wide del catálogo (38 códigos desde `V12`) para leer el audit log |
| `RESOURCE_NOT_FOUND` | 404 | Ruta sin mapping |
| `ORGANIZATION_CODE_ALREADY_EXISTS` | 409 | Code de organización duplicado (global) |
| `BRANCH_CODE_ALREADY_EXISTS` | 409 | Code de branch duplicado en la organización |
| `CATEGORY_NOT_FOUND` | 404 | Categoría inexistente (o de otra organización) |
| `CATEGORY_CODE_ALREADY_EXISTS` | 409 | Code de categoría duplicado dentro de la organización |
| `CATEGORY_INACTIVE` | 409 | No se asigna una categoría INACTIVE a un producto |
| `PRODUCT_NOT_FOUND` | 404 | Producto inexistente (o de otra organización) |
| `PRODUCT_SKU_ALREADY_EXISTS` | 409 | `sku` duplicado dentro de la organización |
| `PRODUCT_BARCODE_ALREADY_EXISTS` | 409 | `barcode` duplicado dentro de la organización |
| `MODIFIER_GROUP_NOT_FOUND` | 404 | Grupo de modifiers inexistente (o de otra organización) |
| `MODIFIER_GROUP_CODE_ALREADY_EXISTS` | 409 | `code` de grupo duplicado dentro de la organización |
| `MODIFIER_OPTION_NOT_FOUND` | 404 | Opción inexistente (o de otro grupo/organización) |
| `MODIFIER_OPTION_CODE_ALREADY_EXISTS` | 409 | `code` de opción duplicado dentro del grupo |
| `CASH_REGISTER_NOT_FOUND` | 404 | Caja inexistente (o de otra branch/organización) |
| `CASH_REGISTER_CODE_ALREADY_EXISTS` | 409 | `code` de caja duplicado dentro de la branch |
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
`GET /organizations/{A}/branches/{B1}`, `GET /organizations/{A}/users/{U1}`,
`GET /organizations/{A}/roles/{R1}`, `GET /organizations/{A}/categories/{C1}`,
`GET /organizations/{A}/products/{P1}` o
`GET /organizations/{A}/modifier-groups/{G1}` o
`GET /organizations/{A}/branches/{B1}/cash-registers/{C1}` donde el recurso
pertenece a otra
organización responde **404** (`BRANCH_NOT_FOUND` / `USER_NOT_FOUND` /
`ROLE_NOT_FOUND` / `CATEGORY_NOT_FOUND` / `PRODUCT_NOT_FOUND` /
`MODIFIER_GROUP_NOT_FOUND` / `CASH_REGISTER_NOT_FOUND`), nunca 200.
Distinción de capas con RBAC:

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
- **Excepción deliberada:** `GET /organizations/{id}/branches` (colección) NO
  está paginado — las sucursales de una organización son pocas por naturaleza y
  su listado completo es el contrato actual. El descubrimiento self-scoped
  `GET /api/v1/auth/me/branches` (paginado, ver abajo) cubre el caso "mis
  sucursales".
- **Self operational context:** `GET /api/v1/auth/me` incluye el contexto
  operativo de la organización (`organization.currency/locale/timezone`,
  derivado del servidor). `GET /api/v1/auth/me/branches` lista, paginado
  (`page`/`size` máx 100, filtros opcionales `status`/`q`, orden `name ASC,
  id ASC`), las sucursales visibles para la identidad: sin `branch.read`
  org-wide solo las de membership ACTIVE del usuario; con `branch.read`
  org-wide todas las de su organización. Es **visibilidad, no autorización**:
  no concede operar, una branch INACTIVE sigue siendo descubrible para sus
  miembros y sin sucursales visibles responde `200` con página vacía.
- Nunca se serializa `Page`/`Pageable` de Spring Data directamente.

## OpenAPI

- `springdoc-openapi` **3.1.1** (compatible con Spring Boot 4.1.x).
- Especificación: `/v3/api-docs`; UI: `/swagger-ui` (solo cuando
  `app.security.permit-opendoc=true`, activo únicamente en `dev`).
- En producción los endpoints de docs siguen **denegados** por
  deny-by-default.

## Otros

- **Optimistic concurrency HTTP**: resuelto como contrato `expectedVersion` en
  Organization + Branch + User + Role + Membership + Category + Product +
  ModifierGroup + ModifierOption + CashRegister + el reemplazo de modifier-groups
  de un producto
  (sin ETag/If-Match por ahora;
  revisar antes de abrir las APIs administrativas en producción).
- **SCOPES**: `hasOrganizationPermission` (organization-wide) vs
  `hasBranchPermission` (org-wide UNION membership branch-scoped) —
  documentado en `docs/04-security/rbac.md`.
- **Idempotencia** (futuro): operaciones monetarias aceptarán
  `Idempotency-Key`. Aún no existe.
- **Seguridad de respuestas:** nunca devolver contraseñas, hashes, tokens,
  secretos ni datos internos de infraestructura.