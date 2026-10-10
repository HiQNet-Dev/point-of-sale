# Modelo User (identidad + administración)

Estado: **implementado** (identidad en `V3__create_users_and_auth_sessions.sql`,
hardening `V4__harden_user_identity.sql`, módulos `users` + `auth`). La
**autenticación** y la **autorización (RBAC + Branch Memberships)** también
están implementadas (`V5`/`V6`).
Versión `0.1.0`.

## Entidad `users`

| Columna | Tipo | Reglas |
| --- | --- | --- |
| `id` | `UUID` PK | Generado en aplicación (`UUID.randomUUID()`) |
| `organization_id` | `UUID` NOT NULL | FK → `organizations(id)`; **toda operación administrativa lo exige** |
| `email` | `VARCHAR(254)` NOT NULL | Normalizado trim+lowercase; formato válido; CHECK `V4` (`email = lower(email) = trim(email)` y no vacío) |
| `password_hash` | `VARCHAR(255)` NOT NULL | Solo hash BCrypt (nunca plaintext); ancho 255 para futuros formatos versionados |
| `status` | `VARCHAR(16)` NOT NULL | CHECK `IN ('ACTIVE','INACTIVE')`; default `ACTIVE` |
| `failed_login_attempts` | `INTEGER` NOT NULL | CHECK `>= 0`; lo gestiona `auth` |
| `locked_until` | `TIMESTAMPTZ` NULL | Lockout temporal gestionado por `auth` |
| `last_login_at` | `TIMESTAMPTZ` NULL | Último login correcto |
| `version` | `BIGINT` NOT NULL | Optimistic locking (`@Version`) |
| `created_at` / `updated_at` | `TIMESTAMPTZ` NOT NULL | UTC (`Instant`) |

Sin columnas de perfil (`first_name`, `phone`, ...) ni soft-delete
(`deleted_at`/`is_deleted`): la baja es `status = INACTIVE`.

## Ownership y unicidad

```text
Organization 1 ── N User   (un usuario pertenece exactamente a una organización)
```

- **Único por organización**, nunca global: `UNIQUE (organization_id, email)`
  (`users_organization_email_unique`). El mismo email puede existir en dos
  organizaciones distintas.
- Mover un usuario entre organizaciones **no está soportado**.

## Administración (API)

```text
POST   /api/v1/organizations/{organizationId}/users          crear
GET    /api/v1/organizations/{organizationId}/users          listar (paginado)
GET    /api/v1/organizations/{organizationId}/users/{userId} obtener
PATCH  /api/v1/organizations/{organizationId}/users/{userId} email y/o status
```

- **Create**: la organización sale SOLO de la ruta; el body acepta `email` y
  `password`. Nunca se aceptan `id`, `organizationId`, `passwordHash`,
  `failedLoginAttempts`, `lockedUntil`, `version`. La organización debe existir
  (**404** `ORGANIZATION_NOT_FOUND`) y estar **ACTIVE** (si no, **409**
  `ORGANIZATION_INACTIVE`).
- **Password inicial**: se hashea con el `PasswordEncoder` compartido (BCrypt);
  el crudo nunca se persiste, loggea ni devuelve. Política (`PasswordPolicy`):
  **12–128 caracteres** (sin reglas de composición artificiales) → **400**
  `WEAK_PASSWORD`.
- **List**: `shared.api.PageResponse` (`items/page/size/totalElements/totalPages`),
  `?page` (0) y `?size` (20, máx 100), orden `email ASC, id ASC`, filtro
  opcional `?status=ACTIVE|INACTIVE`.
- **PATCH** realmente parcial (solo campos presentes); campos inmutables no
  expuestos. Email mal formado → **400** `VALIDATION_ERROR`; email duplicado →
  **409** `USER_EMAIL_ALREADY_EXISTS`; modificación concurrente → **409**
  `CONCURRENT_MODIFICATION`.
- Nunca hay `DELETE`: la baja es `status = INACTIVE`.

El `UserResponse` expone `id, organizationId, email, status, lastLoginAt,
version, createdAt, updatedAt` — **nunca** password/hash, refresh sessions ni
token hashes. `version` se incluye por consistencia con `BranchResponse`.

## Aislamiento (IDOR)

Toda operación administrativa resuelve por `findByIdAndOrganizationId`; pedir
un usuario de otra organización devuelve **404 `USER_NOT_FOUND`** (nunca 200 ni
403), igual que el patrón de `branch`.

## Integración con `auth`

- Desactivar (`ACTIVE → INACTIVE`) es un **límite de seguridad**: además de
  bloquear login/refresh/`/me`, revoca **todas** las refresh sessions del
  usuario (todas las familias/dispositivos) **en la misma transacción** que el
  cambio de status (lock `user` primero → sesiones). La revocación se delega a
  un port de aplicación (`UserSessionRevocationPort` en `users.application`)
  implementado por `auth` (Dependency Inversion).
- Reactivar (`INACTIVE → ACTIVE`) **no** revive ninguna sesión revocada ni crea
  sesiones nuevas: el usuario debe autenticarse de nuevo.
- Dirección de dependencia: **`users` nunca compila contra `auth`** (solo usa el
  port); `auth` implementa el port. Sin ciclo de módulos.

## Migración

- `V4__harden_user_identity.sql` (aditiva) añade el CHECK de normalización de
  email. `V3` es inmutable (ya publicada). `ddl-auto=validate`: Flyway es la
  única fuente de schema.

## Estado del módulo (límites honestos)

```text
User identity            ✔
User administration      ✔
Roles / Permissions      ✔ (módulo rbac, ver docs/04-security/rbac.md)
Branch Memberships       ✔ (módulo membership, ver docs/02-database/branch-memberships-model.md)
```

La **API administrativa** requiere autenticación y el permiso correspondiente
(RBAC) para operar; `POST /organizations` y lo no definido permanecen
**deny-by-default**. El **Secure Admin Bootstrap** (provisión segura del primer
administrador, one-shot transaccional gated por `BOOTSTRAP_SECRET`) está
implementado en el módulo `bootstrap` (V8); no hay credenciales por defecto ni
`SUPER_ADMIN`. Detalle: `docs/04-security/secure-bootstrap.md`.

## Decisión transversal (resuelta en RBAC)

- **HTTP optimistic concurrency contract**: resuelto como `expectedVersion`
  (requerido, `>= 0`) en los PATCH de **Organization, Branch, User y Role**
  (v0.1.x, sin ETag/If-Match todavía). Con Branch Memberships, `PATCH
  /memberships/{userId}` también lo exige.
