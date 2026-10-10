# Módulos planeados

**Organization, Branch y Authentication (auth) están implementados.** El
resto de los módulos son previstos; cada uno se marca como **no
implementado** hasta que llegue su milestone (ver
`docs/00-product/roadmap.md`).

## Tabla de módulos

| Módulo | Responsabilidad | Conceptos clave | Estado |
| --- | --- | --- | --- |
| `auth` | Autenticación y sesiones | login, refresh rotativo, logout, /me | **implementado** |
| `organization` | Empresas/organizaciones | organization, scoping de catálogo | **implementado** |
| `branch` | Sucursales | branch, scoping de existencias | **implementado** |
| `users` | Usuarios y permisos | users (identidad + administración por organización), roles, RBAC | **parcial (identidad + administración)** |
| `rbac` | Roles, permisos y autorización | permissions (catálogo), roles por organización, assignments, method security | **implementado** |
| `membership` | Memberships por sucursal | branch_memberships, branch_membership_roles, scope branch-scoped, port de acceso | **implementado** |
| `audit` | Auditoría persistente append-only | audit_events, actor USER/SYSTEM, trigger DB, port transversal | **implementado (Foundation)** |
| `bootstrap` | Provisión one-shot del primer administrador | installation_bootstrap (singleton), X-Bootstrap-Token, provisioning transaccional auditado | **implementado (Foundation)** |
| `catalog` | Catálogo comercial | products, variants, precios, ingredientes | **no implementado** |
| `sales` | Venta y tickets | sales, tickets, líneas, totales | **no implementado** |
| `payments` | Pagos de la venta | methods, splits, vuelto | **no implementado** |
| `cash` | Cajas y turnos | cash registers, shifts, arqueos | **no implementado** |
| `inventory` | Existencias e insumos | stock por branch, ingredients, recipes, mermas | **no implementado** |
| `purchasing` | Compras | suppliers, órdenes, recepción | **no implementado** |
| `expenses` | Gastos del negocio | expenses, categorías | **no implementado** |
| `reporting` | Reportes | ventas, inventario, caja | **no implementado** |
| `shared` | Utilidades transversales | config, error (`ApiError`, `BusinessException`, `ErrorCodes`) | **parcial (fundación)** |

## Convención de paquetes

```text
com.hiqnet.pos.<modulo>[.domain|.application|.infrastructure|.api]
```

- Split `domain/application/infrastructure/api` **opcional**: solo se usa cuando
  el módulo lo justifica; en módulos pequeños se mantiene mínimo.
- `organization` y `branch` usan `domain` (entidades, enums, repositorios
  Spring Data), `application` (servicios y commands) y `api` (controllers y
  DTOs). **No** hay `infrastructure` propio: Spring Data genera la
  implementación de repositorios, no existe código de persistencia manual.
- `auth` añade `infrastructure` cuando hay lógica técnica no de dominio:
  `RefreshTokenHasher` (generación criptográfica del token opaco y su hash
  SHA-256). El resto sigue `domain/application/api`.
- `users` usa `domain` (`User`, `UserRepository`, `UserStatus`), `application`
  (`UserService`, `PasswordPolicy`, commands) y `api` (`UserController`, DTOs).
  Sin `infrastructure`.
- `rbac` usa `domain` (`Permission`, `Role`, `RolePermission`,
  `UserOrganizationRole` + repos), `application` (`RoleService`,
  `RolePermissionService`, `UserRoleService`, `AuthorizationService`,
  `BranchMembershipAccessPort`) y `api`
  (`PermissionController`, `RoleController`, `RolePermissionController`,
  `UserRoleController`).
- `membership` usa `domain` (`BranchMembership`, `BranchMembershipRole`,
  `BranchMembershipStatus` + repos), `application` (`BranchMembershipService`,)
  e `infrastructure` (adaptador del port de autorización). El port
  `BranchMembershipAccessPort` se declara en `rbac.application` y lo
  implementa el adaptador en `membership.infrastructure` (patrón de inversión
  de dependencia como la revocación de sesiones):
  `membership -> rbac` es la única dirección de compilación, sin ciclo.
- `audit` usa `domain` (`AuditEvent` inmutable + `AuditEventRepository`
  restringido: solo `save`/query, sin delete), `infrastructure`
  (`AuditRecorder`, implementa `shared.audit.AuditPort` resolviendo el actor
  desde `SecurityContext` + `Clock`), `application` (`AuditQueryService` con
  Specifications) y `api` (lectura). `shared.audit` solo define el contrato
  (`AuditPort`, `AuditRecord`, `AuditMetadata`, `AuditException`).
- `bootstrap` usa `domain` (`InstallationBootstrap` singleton + repositorio con
  `PESSIMISTIC_WRITE`), `application` (`BootstrapService` orquestador que
  REUTILIZA `OrganizationService`, `UserService`, `RoleService`,
  `RolePermissionService` y `UserRoleService` — todos `REQUIRED`, se unen a la
  transacción exterior) y `api` (`BootstrapController` + DTOs). La validación
  del secret y el guard HTTP residen en `shared.config`
  (`BootstrapProperties`, `BootstrapTokenGuard`, `BootstrapTokenFilter`) para
  evitar `shared -> bootstrap`; la dirección es única:
  `bootstrap -> shared, organization, users, rbac` — ningún módulo
  fundacional depende de `bootstrap`.
- Dependencia entre módulos (lectura): `branch` usa
  `OrganizationRepository` de `organization` para validar existencia y
  resolución de timezone; `auth` usa `User`/`UserRepository` de `users` y
  `OrganizationRepository` para resolver el login; `users` usa
  `OrganizationRepository` y `shared`; `rbac` usa `users` (User/UserRepository),
  `organization` y `shared`; `membership` usa `organization`, `branch`, `users`
  y `rbac` (roles + port). Dirección única, sin ciclos:
  `organization` no conoce a nadie, **`users` nunca depende de `auth`** y
  **`rbac`/`membership` no dependen de `auth`** (auth reacciona a `User.status`; la
  autorización la evalúa `rbac`/`membership` contra PostgreSQL, no el JWT).
  **Todos los módulos de negocio dependen únicamente de `shared.audit`** (nunca
  de `audit`); `audit` implementa el port y solo lee de repositorios ajenos
  para validación de filtros — sin ciclos, sin imports de Spring Security en
  los services de negocio por causa de auditoría.
- Compartido actual: `com.hiqnet.pos.shared.config` (`SecurityConfig`,
  `CorsConfig`, `AuthConfig`, `AuthProperties`) y `com.hiqnet.pos.shared.error`.

## Regla de scoping (repetida a propósito)

- **Catálogo → organización:** productos, variantes e ingredientes se definen a
  nivel de organización.
- **Existencias → sucursal:** el stock nunca es global cuando hay varias
  sucursales; `inventory` siempre resuelve por branch.
- **Aislamiento de branch:** toda operación sobre una branch pasa por
  `organization_id` (`findByIdAndOrganizationId`); los ids de branch nunca se
  resuelven de forma global. Convención futura del sistema.

## Estado actual del código

Existen `com.hiqnet.pos.shared`, los módulos `organization` y `branch`
(implementados: migraciones Flyway `V1`/`V2`, entidades, servicios,
`POST/GET/PATCH` bajo `/api/v1`, tests de integración con Testcontainers), el
módulo `users` (identidad **y administración**: `create/get/list paginado/patch`,
`UserService`, `PasswordPolicy`, scoping por organización, migración Flyway
`V4`) y el módulo `auth` (migración Flyway `V3`, `AuthService`, refresh rotativo,
`AuthController`, JWT, hardening de concurrencia de sesiones y de validación de
claims/organización), el módulo `rbac` (migración `V5`, catálogo de
permissions, roles por organización, assignments, `AuthorizationService`
database-authoritative, `@EnableMethodSecurity`, `expectedVersion`), el módulo
`membership` (migración `V6`, `BranchMembership` + `BranchMembershipRole`,
CRUD de memberships, replace atómico de roles branch-scoped, port
`BranchMembershipAccessPort` + `hasBranchPermission`, catálogo de permissions
14→18) y el módulo `audit` (migración `V7`, `audit_events` append-only con
trigger DB, actor USER/SYSTEM central, port `shared.audit.AuditPort` con
`MANDATORY`, metadata JSONB segura/acotada, endpoint de lectura con
`audit.read`, catálogo 18→19), el módulo `bootstrap` (migración `V8`,
`installation_bootstrap` singleton con lock pesimista, guard `X-Bootstrap-Token`
constant-time en `shared.config`, provisioning one-shot transaccional y
auditado con actor SYSTEM) y **346
  tests verdes** (Testcontainers). Las clases de arranque siguen en
su lugar. La API administrativa (organizations GET/PATCH, branches,
users, roles, permissions, memberships, audit-events) está **autorizada por
permisos** con PostgreSQL como
autoridad; `POST /organizations` y todo lo no definido permanecen
**deny-by-default** (el provisioning one-shot vive en `POST /api/v1/bootstrap`,
gated por `BOOTSTRAP_SECRET`; véase
`docs/04-security/security-model.md`, `docs/04-security/rbac.md`,
`docs/04-security/audit.md`, `docs/04-security/secure-bootstrap.md` y
`docs/02-database/branch-memberships-model.md`).