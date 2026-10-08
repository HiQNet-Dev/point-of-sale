# Roles / Permissions / RBAC

**Estado:** implementado (v0.1.x Foundation). Autorización **database-authoritative**:
el JWT es identidad únicamente y PostgreSQL es la autoridad cada petición
(ADR `docs/01-architecture/adr/0005-database-authoritative-rbac.md`).

## Modelo

```text
User
  ↓ user_organization_roles (organization_id, user_id, role_id)
Role  (per Organization; code slug único e inmutable; status ACTIVE|INACTIVE)
  ↓ role_permissions (role_id, permission_code)
Permission (catálogo global, sembrado por V5; sin CRUD de cliente)
```

- **Permissions**: catálogo del SISTEMA (`permissions.code` PK natural).
  Se crean únicamente vía Flyway (`V5`). Orden `code ASC`.
- **Role**: pertenece a una organización; `code` normalizado (trim+lowercase,
  slug) único por organización e inmutable; `name` editable; sin DELETE
  (baja = `status INACTIVE`).
- **RolePermission**: grant global → rol (unique `role_id, permission_code`).
- **UserOrganizationRole**: `User posee este Role para toda la Organization`.
  **PostgreSQL impide** asignar un rol de organización B a un usuario de A
  mediante FKs compuestas (`UNIQUE (id, organization_id)` en users/roles).
- **Borradores del alcance por sucursal** (Branch Memberships) NO están
  implementados: RBAC es organization-wide por ahora.

## Permisos efectivos

Permiso efectivo cuando existe: User ACTIVE y de la organización, Organization
ACTIVE, por lo menos un Role ACTIVE asignado al usuario, y `role_permissions`
contiene el código solicitado. Con varios roles: **UNION**. Sin deny rules,
jerarquías, wildcards ni herencia.

`AuthorizationService.hasPermission(authentication, organizationId, code)`
(usado por `@PreAuthorize`) lo evalúa en una query `EXISTS` (sin N+1, sin cache,
sin `securityVersion`).

## Method security

- `@EnableMethodSecurity` + `@PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'permiso')")`.
- Se usan **permission codes**, nunca `hasRole("ADMIN")`.

## Postura de `SecurityConfig`

- Públicos/auth sin cambios (`/auth/login|refresh|logout`, `/me`, health/docs).
- Rutas administrativas concretas → `authenticated()` (filtro); la autorización
  fina → method security (`@PreAuthorize`).
- **`POST /api/v1/organizations` sigue cerrado** (no hay `organization.create`):
  crear organizaciones será bootstrap seguro / Control Plane.
- `anyRequest().denyAll()` cierra todo lo no definido.

## Matriz de permisos

| Endpoint | Permiso |
| --- | --- |
| GET /organizations/{id} | `organization.read` |
| PATCH /organizations/{id} | `organization.update` |
| GET /organizations/{id}/branches | `branch.read` |
| POST /organizations/{id}/branches | `branch.create` |
| GET /organizations/{id}/branches/{branchId} | `branch.read` |
| PATCH /organizations/{id}/branches/{branchId} | `branch.update` |
| GET /organizations/{id}/users | `user.read` |
| POST /organizations/{id}/users | `user.create` |
| GET /organizations/{id}/users/{userId} | `user.read` |
| PATCH /organizations/{id}/users/{userId} | `user.update` |
| GET /organizations/{id}/permissions | `permission.read` |
| GET /organizations/{id}/roles | `role.read` |
| POST /organizations/{id}/roles | `role.create` |
| GET /organizations/{id}/roles/{roleId} | `role.read` |
| PATCH /organizations/{id}/roles/{roleId} | `role.update` |
| GET /organizations/{id}/roles/{roleId}/permissions | `role.read` |
| PUT /organizations/{id}/roles/{roleId}/permissions | `role.permissions.manage` |
| GET /organizations/{id}/users/{userId}/roles | `role.read` |
| PUT /organizations/{id}/users/{userId}/roles | `role.assign` |

## Inmediatez (mismo JWT)

Verificado por tests e2e con el MISMO access token: quitar un permiso,
quitar/poner una asignación de rol, desactivar/reactivar un rol, o pasar
User/Organization a INACTIVE se refleja en la **siguiente petición** (200 → 403
→ 200 según aplique). Los access JWTs no se blacklistean (stateless, 10m).

## Límites de respuesta

- Sin token → `401 AUTHENTICATION_REQUIRED`.
- Token válido sin el permiso (o User/Organization/rol no operativos) → **403
  `FORBIDDEN`**.
- Violación de scope de autorización (usuario con permisos solo en A que pide
  recursos de B) → **403** (la autorización ocurre primero).
- Aislamiento de recurso (id ajeno con permiso válido en la org de la ruta) →
  **404** (`USER_NOT_FOUND` / `BRANCH_NOT_FOUND` / `ROLE_NOT_FOUND`).

## Optimistic concurrency HTTP

Promovido de decisión pendiente a contrato v0.1.x: **`expectedVersion`**
(requerido, `>= 0`) en PATCH de Organization, Branch, User y Role. El servidor
compara con `@Version`; mismatch → `409 CONCURRENT_MODIFICATION`. Sin ETag /
If-Match todavía.

## Roles INACTIVE

Un rol `INACTIVE` **no concede** permissions; los assignments permanecen y al
volver `ACTIVE` vuelven a conceder (intencional; documentado).

## No implementado (fronteras)

- **Branch Memberships** (rol por sucursal) — siguiente slice.
- **Audit persistente** (el logging estructurado `security-event=role-*` no es
  auditoría).
- **Secure Admin Bootstrap** (primer ADMIN) — pendiente de Foundation; sin
  credenciales por defecto.
- **SUPER_ADMIN / ROOT / HIQNET_ADMIN** dentro del POS: la administración
  HiQNet vive en un futuro Control Plane separado (`Business Administration !=
  HiQNet Administration`).
- **Frontend**, cache de permisos, `securityVersion`, blacklist JWT.