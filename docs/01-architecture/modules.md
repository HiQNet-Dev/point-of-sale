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
| `catalog` | Catálogo comercial | categorías, productos y modifiers Organization-owned; lecturas administrativas y Operational POS Catalog con Branch como contexto | **implementado** |
| `cashregister` | Cajas físicas (branch-owned) | cash_registers, slug único por branch, scope branch-scoped | **implementado (Foundation)** |
| `cashshift` | Turnos de caja (register-owned) | cash_shifts, ciclo OPEN→CLOSED, un turno OPEN por caja (índice único parcial), scope branch-scoped | **implementado (Foundation)** |
| `sales` | Venta operacional | sales, sale_items, modifier snapshots, pricing server-authoritative, OPEN→CANCELLED/COMPLETED | **implementado** |
| `payment` | Pagos inmutables de checkout | CASH/CARD, split tender, tendered/change, historical reads | **implementado** |
| `cash` | Turnos de caja y arqueos | shifts, arqueos (cash registers viven en `cashregister`) | **no implementado** |
| `inventory` | Definiciones y consumo teórico de inventario; existencias operativas por sucursal | `InventoryItem` organization-owned (V16); Branch balances and immutable movements (V17); Product recipes and ModifierOption effects organization-owned (V18 candidate / PR open) | **Inventory foundation in progress** |
| `purchasing` | Compras | suppliers, órdenes, recepción | **no implementado** |
| `expenses` | Gastos del negocio | expenses, categorías | **no implementado** |
| `reporting` | Reportes | ventas, inventario, caja | **no implementado** |
| `shared` | Utilidades transversales | config, error (`ApiError`, `BusinessException`, `ErrorCodes`) | **parcial (fundación)** |

## Frontend authentication/session

El frontend mantiene la sesión bajo `src/features/auth`: `api/authApi.ts` adapta
los contratos HTTP reales, `model/authTokenStore.ts` conserva exclusivamente
los tokens en memoria, `model/AuthSessionProvider.tsx` coordina sesión y
`/auth/me`, y `components`/`pages` contienen los guards y pantallas mínimas.
El cliente común `src/shared/api/client.ts` agrega `Authorization: Bearer`
centralmente y es el único responsable del refresh automático.

El backend entrega `accessToken` y `refreshToken` en JSON, no cookies. Por ello
ambos permanecen solo en memoria: no se escriben en localStorage,
sessionStorage, IndexedDB ni TanStack Query. Un 401 comparte una sola promesa de
refresh entre requests concurrentes, el refresh opaco rota, y cada request
original se reintenta como máximo una vez. Un refresh inválido o un segundo 401
limpia la sesión y el QueryClient. Login solo se considera completo después de
que `/auth/me` confirma la identidad y el contexto Organization. Logout toma una
copia efímera del refresh token, invalida y limpia el estado/cache local, y luego
intenta revocarlo una sola vez en backend; un fallo remoto no restaura la sesión.
Si la llamada de revocación no llega al servidor, la sesión de refresh remota
puede seguir válida hasta su expiración; el cliente no conserva el token para
reintentar después.

Al recargar la página se pierde el refresh token en memoria y no se intenta
reconstruir la sesión; el usuario vuelve a `/login`. Persistirlo requeriría un
cambio backend coordinado a cookie HttpOnly/Secure/SameSite y su política CORS,
no una degradación de almacenamiento desde el frontend. El frontend no decodifica
JWT para permisos ni mantiene roles, branches o un snapshot de autorización.

## Frontend POS session

`src/features/pos-session` compone las APIs existentes para reconstruir el
contexto operativo en `/pos`: consulta `/auth/me/branches` sin filtrar Branches
inactivas, luego obtiene Registers y turnos OPEN dentro del scope seleccionado.
Los IDs usados en cada paso provienen de respuestas anteriores; no se guardan
selecciones ni montos en almacenamiento del navegador. Branch discovery no
concede permisos: los errores de lectura/apertura/cierre del backend se
presentan en la etapa del recurso correspondiente. Una Branch o Register
inactiva puede conservar un turno OPEN y permitir su cierre, pero nunca habilita
la apertura de uno nuevo. El workspace de ventas consume este contexto sin
crear ventas automáticamente. Reanuda OPEN Sales y crea una venta solo mediante
acción explícita. El Sale detail es el carrito persistido y la autoridad de precios;
las mutaciones usan la versión actual devuelta por el servidor. Checkout registra
instrucciones CASH/CARD efímeras en una sola operación atómica. La UI no consulta
historial de pagos para confirmar éxito ni implementa procesamiento de tarjetas.

## Frontend POS sales

`src/features/pos-sales` consume los endpoints branch-context de Sales y
Operational Catalog con query keys que incluyen Organization, Branch, Register y
Shift. Solo la acción explícita “Nueva venta” crea una Sale; una OPEN Sale única se
reanuda automáticamente y múltiples OPEN Sales se pueden seleccionar. El Sale
detail y sus snapshots/totales son la única fuente del carrito. Catálogo y cambios
de líneas se deshabilitan con Branch/Register inactivos, pero la venta recuperada
se puede leer y cancelar. No se almacenan carrito, selección de venta ni permisos
en browser storage; catálogo y permisos siempre se vuelven a consultar al backend.
Checkout UI está implementado. Sus borradores CASH/CARD no se persisten en browser
storage ni crean Payments antes del POST atómico. El cálculo exacto de
importes/restante/cambio es solo previsualización UX; backend valida cobertura,
cambio y estado final. No se consulta `payment.read` al confirmar, ni se captura
información sensible de tarjeta o se procesa una tarjeta. La verificación E2E
integrada (navegador + backend + PostgreSQL) está en progreso.

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
- `catalog` usa `domain` (`Category`, `Product`, `ModifierGroup`,
  `ModifierOption`, `ProductModifierGroup`, sus enums + repositorios Spring Data;
  `ProductRepository`/`ModifierGroupRepository` con `JpaSpecificationExecutor`),
  `application` (`CategoryService`, `ProductService`, `ModifierGroupService`,
  `ModifierOptionService`, `ProductModifierService`, commands) y `api`
  (`CategoryController`, `ProductController`, `ModifierGroupController`,
  `ModifierOptionController`, `ProductModifierGroupController`,
  `OperationalCatalogController` y DTOs). `OperationalCatalogService` usa
  Branch exclusivamente como contexto de autorización (`pos.catalog.read`, Branch
  ACTIVE); Category/Product/Modifier siguen Organization-owned. Dependencias de
  lectura: `catalog -> organization, branch` (solo la superficie operacional) y
  `shared`; no existe pertenencia ni precio por Branch.
- `cashregister` usa `domain` (`CashRegister`, `CashRegisterStatus` +
  `CashRegisterRepository` con `JpaSpecificationExecutor`), `application`
  (`CashRegisterService`, `UpdateCashRegisterCommand`) y `api`
  (`CashRegisterController` + DTOs). Sin `infrastructure`. Dependencia de lectura:
  `cashregister -> branch, organization` (valida existencia y deriva el
  `organization_id` de la branch) y `shared`; el recurso es **branch-owned**.
- `cashshift` usa `domain` (`CashShift`, `CashShiftStatus` + `CashShiftRepository`), `application` (`CashShiftService`, `OpenSalesForCashShiftPort`) y `api` (`CashShiftController` + DTOs). `sales` implementa el port de guard para evitar que cashshift dependa de un repositorio concreto de Sales.
- `sales` usa `domain` (`Sale`, `SaleItem`, `SaleItemModifier`, estados y repositorios), `application` (`SaleService`, adaptador del guard de turno) y `api` (`SaleController` y DTOs). Depende de catálogo para cotizar snapshots y de los recursos operacionales para scope; create, checkout and close share the scoped CashShift coordination lock (ADR-0007/0008), while regular mutations retain `Sale.version` optimistic. `payment` contains only immutable Payment facts/repository and response DTO; it has no independent write API.
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
auditado con actor SYSTEM) y el módulo `catalog` (migraciones `V9`/`V10`,
`Category` + `Product` organization-owned, `POST/GET/PATCH` scoped por
organización, filtros de producto `?status&categoryId&q`, FK compuesta same-org
producto→categoría, SKU canónico inmutable, barcode único parcial; y el modelo de
**modifiers** `V10`: `ModifierGroup`/`ModifierOption` reutilizables + relación
`ProductModifierGroup` ordenada con FK compuestas same-org y replace atómico
`product.modifiers.manage`; catálogo 19→25→32) y el módulo `cashregister`
(migración `V11`, `CashRegister` **branch-owned**, slug `code` único por branch e
inmutable, `name` no-blank, `status ACTIVE/INACTIVE`, FK compuesta same-org
`(branch_id, organization_id) → branches`, autorización branch-scoped
`cash-register.*`; catálogo 32→35) y el módulo `cashshift` (migración `V12`,
`CashShift` **register-owned**, ciclo `OPEN → CLOSED`, actores y timestamps
derivados del servidor, montos declarados `NUMERIC(19,4)`, un solo turno `OPEN`
por caja vía índice único parcial, FKs compuestas same-org/same-branch,
autorización branch-scoped `cash-shift.*`, auditoría append-only en open/close;
catálogo 35→38) y **566 tests verdes** (Testcontainers). Las clases de arranque siguen en
su lugar. La API administrativa (organizations GET/PATCH, branches,
users, roles, permissions, memberships, categories, products, modifier-groups,
modifier-options, product modifier-groups, cash-registers, cash-shifts, audit-events) está **autorizada por
permisos** con PostgreSQL como
autoridad; `POST /organizations` y todo lo no definido permanecen
**deny-by-default** (el provisioning one-shot vive en `POST /api/v1/bootstrap`,
gated por `BOOTSTRAP_SECRET`; véase
`docs/04-security/security-model.md`, `docs/04-security/rbac.md`,
`docs/04-security/audit.md`, `docs/04-security/secure-bootstrap.md`,
`docs/02-database/catalog-model.md`,
`docs/02-database/product-modifiers-model.md` y
`docs/02-database/cash-register-model.md` y
`docs/02-database/cash-shift-model.md`).