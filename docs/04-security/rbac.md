# Roles / Permissions / RBAC

**Estado:** implementado (v0.1.x Foundation). Autorización **database-authoritative**:
el JWT es identidad únicamente y PostgreSQL es la autoridad cada petición
(ADR `docs/01-architecture/adr/0005-database-authoritative-rbac.md`).

## Modelo

```text
User
  ↓ user_organization_roles (organization_id, user_id, role_id)     [ORGANIZATION scope]
Role  (per Organization; code slug único e inmutable; status ACTIVE|INACTIVE)
  ↓ role_permissions (role_id, permission_code)
Permission (catálogo, sembrado por V5/V6/V7/V9/V10/V11/V12/V13; sin CRUD de cliente)

User
  ↓ branch_memberships (organization_id, branch_id, user_id, status)  [BRANCH scope]
  ↓ branch_membership_roles (organization_id, membership_id, role_id)
Role   (mismas definiciones de roles; sin modelo BranchRole separado)
```

- **Permissions**: catálogo del SISTEMA (`permissions.code` PK natural).
  Se crean únicamente vía Flyway (`V5`/`V6`/`V7`/`V9`/`V10`/`V11`/`V12`/`V13`). Orden `code ASC`.
- **Role**: pertenece a una organización; `code` normalizado (trim+lowercase,
  slug) único por organización e inmutable; `name` editable; sin DELETE
  (baja = `status INACTIVE`).
- **RolePermission**: grant global → rol (unique `role_id, permission_code`).
- **UserOrganizationRole**: `User posee este Role para toda la Organization`.
  **PostgreSQL impide** asignar un rol de organización B a un usuario de A
  mediante FKs compuestas (`UNIQUE (id, organization_id)` en users/roles).
- **BranchMembership** (`V6`) y **BranchMembershipRole** (`V6`): vínculo
  explícito User→Branch con roles branch-scoped (una membership por
  org+branch+user; `status ACTIVE/INACTIVE`; `@Version`). Los roles reutilizan
  el catálogo `roles`; FKs compuestas impiden membership/rol cross-org. Detalle:
  `docs/02-database/branch-memberships-model.md`.

## Permisos efectivos

Permiso efectivo cuando existe: User ACTIVE y de la organización, Organization
ACTIVE, por lo menos un Role ACTIVE asignado al usuario, y `role_permissions`
contiene el código solicitado. Con varios roles: **UNION**. Sin deny rules,
jerarquías, wildcards ni herencia.

`AuthorizationService.hasPermission(authentication, organizationId, code)`
(usado por `@PreAuthorize`) lo evalúa en una query `EXISTS` (sin N+1, sin cache,
sin `securityVersion`).

## Scopes: ORGANIZATION vs BRANCH

Existen dos scopes de autorización explícitos:

- **ORGANIZATION scope** — `hasPermission(authentication, orgId, code)`:
  el usuario tiene el permiso organization-wide vía `user_organization_roles`.
- **BRANCH scope** — `hasBranchPermission(authentication, orgId, branchId, code)`:
  concede si el usuario tiene el permiso organization-wide (override) **O** una
  membership ACTIVE de esa branch cuyo rol ACTIVE contenga el permiso (con
  Branch ACTIVE).

```text
hasBranchPermission = organización (override) UNION branch membership
NUNCA: branch role -> organización (anti privilege-escalation)
```

Los endpoints organization-wide SIEMPRE usan `hasPermission`; solo los detalles
`GET/PATCH /branches/{branchId}` usan `hasBranchPermission` (un Branch Manager
de su sucursal puede operarla, sin tocar otras). La branch membership leg se
avala en una sola `EXISTS` (`BranchMembershipRoleRepository`
`.existsEffectiveBranchPermission`) que exige User/Organization/Branch/ Membership/Role ACTIVE y misma organización.

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
| GET /organizations/{id}/branches/{branchId} | `branch.read` — `hasBranchPermission(…#branchId…)` |
| PATCH /organizations/{id}/branches/{branchId} | `branch.update` — `hasBranchPermission(…#branchId…)` |
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
| POST /organizations/{id}/branches/{branchId}/memberships | `branch.membership.create` (org scope) |
| GET /organizations/{id}/branches/{branchId}/memberships | `branch.membership.read` (org scope) |
| GET /organizations/{id}/branches/{branchId}/memberships/{userId} | `branch.membership.read` (org scope) |
| PATCH /organizations/{id}/branches/{branchId}/memberships/{userId} | `branch.membership.update` (org scope) |
| GET /organizations/{id}/branches/{branchId}/memberships/{userId}/roles | `branch.membership.read` (org scope) |
| PUT /organizations/{id}/branches/{branchId}/memberships/{userId}/roles | `branch.membership.roles.manage` (org scope) |
| GET /organizations/{id}/audit-events | `audit.read` (org scope; SOLO lectura) |
| GET /organizations/{id}/categories | `category.read` (org scope) |
| POST /organizations/{id}/categories | `category.create` (org scope) |
| GET /organizations/{id}/categories/{categoryId} | `category.read` (org scope) |
| PATCH /organizations/{id}/categories/{categoryId} | `category.update` (org scope) |
| GET /organizations/{id}/products | `product.read` (org scope; filtros `?status&categoryId&q`) |
| POST /organizations/{id}/products | `product.create` (org scope) |
| GET /organizations/{id}/products/{productId} | `product.read` (org scope) |
| PATCH /organizations/{id}/products/{productId} | `product.update` (org scope) |
| POST /organizations/{id}/modifier-groups | `modifier.group.create` (org scope) |
| GET /organizations/{id}/modifier-groups | `modifier.group.read` (org scope; filtros `?status&q`) |
| GET /organizations/{id}/modifier-groups/{groupId} | `modifier.group.read` (org scope) |
| PATCH /organizations/{id}/modifier-groups/{groupId} | `modifier.group.update` (org scope) |
| POST /organizations/{id}/modifier-groups/{groupId}/options | `modifier.option.create` (org scope) |
| GET /organizations/{id}/modifier-groups/{groupId}/options | `modifier.option.read` (org scope; filtro `?status`) |
| GET /organizations/{id}/modifier-groups/{groupId}/options/{optionId} | `modifier.option.read` (org scope) |
| PATCH /organizations/{id}/modifier-groups/{groupId}/options/{optionId} | `modifier.option.update` (org scope) |
| GET /organizations/{id}/products/{productId}/modifier-groups | `product.read` (org scope) |
| PUT /organizations/{id}/products/{productId}/modifier-groups | `product.modifiers.manage` (org scope) |
| POST /organizations/{id}/branches/{branchId}/cash-registers | `cash-register.create` (branch scope) |
| GET /organizations/{id}/branches/{branchId}/cash-registers | `cash-register.read` (branch scope; filtros `?status&q`) |
| GET /organizations/{id}/branches/{branchId}/cash-registers/{cashRegisterId} | `cash-register.read` (branch scope) |
| PATCH /organizations/{id}/branches/{branchId}/cash-registers/{cashRegisterId} | `cash-register.update` (branch scope) |
| POST /organizations/{id}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts | `cash-shift.open` (branch scope) |
| GET /organizations/{id}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts | `cash-shift.read` (branch scope; filtro `?status`) |
| GET /organizations/{id}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId} | `cash-shift.read` (branch scope) |
| POST /organizations/{id}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/close | `cash-shift.close` (branch scope) |
| GET sale list/detail | `sale.read` (branch recovery scope) |
| POST sale | `sale.create` (branch scope) |
| POST/PATCH/remove sale item | `sale.update` (branch scope) |
| POST sale cancel | `sale.cancel` (branch recovery scope) |
| POST /api/v1/bootstrap | provisioning one-shot — `X-Bootstrap-Token` (secret), SIN permiso; `organization.create` sigue sin existir |

## Inmediatez (mismo JWT)

Verificado por tests e2e con el MISMO access token: quitar un permiso,
quitar/poner una asignación de rol, desactivar/reactivar un rol, o pasar
User/Organization a INACTIVE se refleja en la **siguiente petición** (200 → 403
→ 200 según aplique). También los cambios de membership (assign/remove roles,
membership INACTIVE, rol INACTIVE, Branch INACTIVE) se reflejan al instante con
el mismo JWT (ADR-0005). Los access JWTs no se blacklistean (stateless, 10m).

## Límites de respuesta

- Sin token → `401 AUTHENTICATION_REQUIRED`.
- Token válido sin el permiso (o User/Organization/rol no operativos) → **403
  `FORBIDDEN`**.
- Violación de scope de autorización (usuario con permisos solo en A que pide
  recursos de B) → **403** (la autorización ocurre primero).
- Aislamiento de recurso (id ajeno con permiso válido en la org de la ruta) →
  **404** (`USER_NOT_FOUND` / `BRANCH_NOT_FOUND` / `ROLE_NOT_FOUND` /
  `CATEGORY_NOT_FOUND` / `PRODUCT_NOT_FOUND` / `MODIFIER_GROUP_NOT_FOUND` /
  `MODIFIER_OPTION_NOT_FOUND` / `CASH_REGISTER_NOT_FOUND`).

## Optimistic concurrency HTTP

Promovido de decisión pendiente a contrato v0.1.x: **`expectedVersion`**
(requerido, `>= 0`) en PATCH de Organization, Branch, User, Role, Membership,
Category, Product, ModifierGroup, ModifierOption y CashRegister, y en el `PUT` de
reemplazo
producto↔modifier-group. El servidor
compara con `@Version`; mismatch → `409 CONCURRENT_MODIFICATION`. Sin ETag /
If-Match todavía.

## Roles INACTIVE

Un rol `INACTIVE` **no concede** permissions; los assignments permanecen y al
volver `ACTIVE` vuelven a conceder (intencional; documentado).

## No implementado (fronteras)

- **SUPER_ADMIN / ROOT / HIQNET_ADMIN** dentro del POS: la administración
  HiQNet vive en un futuro Control Plane separado (`Business Administration !=
  HiQNet Administration`).
- **Frontend**, cache de permisos, `securityVersion`, blacklist JWT.
- **Branch-scoped listing «mis sucursales»**: implementado como
  `GET /api/v1/auth/me/branches` (self-scoped, paginado). Sin `branch.read`
  org-wide devuelve solo las sucursales con membership ACTIVE del usuario; con
  `branch.read` org-wide, todas las de su organización. Es **visibilidad, no
  autorización**: no promueve a scope de organización, no requiere
  `branch.membership.read`, y una branch INACTIVE sigue siendo descubrible para
  sus miembros (recovery). `GET /branches` (colección) sigue siendo
  organization-scope.
- **Read-recovery tras desactivar una branch**: las lecturas de Cash Register y
  Cash Shift (`GET` list/detail) y el `close` de Cash Shift usan
  `hasBranchPermissionIgnoringBranchStatus` (la branch leg no exige branch
  ACTIVE), para que un recurso ya abierto siga siendo recuperable. `open` y las
  escrituras de Cash Register siguen exigiendo la branch ACTIVE.
- **Recovery/backdoor** para el administrador destruido: no (operativo/futuro).