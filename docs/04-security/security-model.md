# Modelo de seguridad

**Estado actual:** autenticación implementada (`auth`), administración de
usuarios implementada (`users`), **Roles/Permissions/RBAC implementado
(`rbac`)**, **Branch Memberships implementado (`membership`)** y **Audit
Foundation implementado (`audit`)** — las business
APIs administrativas abren **solo** con el permiso
adecuado. Secure Admin Bootstrap y Control Plane siguen siendo futuros.

## Principios

1. **Deny by default.** Todo lo no explícitamente permitido está denegado.
2. **Least privilege.** Cada rol/entidad ve y hace lo mínimo necesario.
3. **Autorización server-side.** El frontend **nunca** es fuente de verdad de
   permisos; solo UX. Cualquier regla crítica se valida en el servidor.
4. **Scoping organization/branch.** Toda consulta de negocio se acota al
   contexto de la organización y, cuando aplica, a la sucursal.
   **Sin fallbacks `branchId=1`, sin IDs mágicos, sin "por defecto la primera sucursal".**
5. **Secretos por entorno.** Contraseñas y credenciales viven en variables de
   entorno por perfil; nunca en Git. `AUTH_JWT_SECRET` (clave Base64) no tiene
   default: la aplicación no arranca sin ella (excepto tests con secreto de
   prueba).
6. **Auditoría.** Quién, cuándo, qué. Operaciones de dinero: inmutables
   (sin hard delete), trazables y transaccionales.
7. **Protección de operaciones de dinero.** Re-autenticación para operaciones
   sensibles (futuro), idempotencia (`Idempotency-Key`, futuro), atomicidad
   transaccional.
8. **CORS por allowlist.** Nunca wildcard (`*`) junto con credenciales;
   la lista se configura por variable (`CORS_ALLOWED_ORIGINS`).
9. **Actuator mínimo.** Solo `health` expuesto, `show-details: never`
   (no filtra host de BD, rutas de disco, etc.).

## Estado actual (fundación)

`SecurityConfig`:

- `permitAll` **solo** en `/actuator/health`, `/error`, docs (si
  `app.security.permit-opendoc`) y los endpoints públicos de auth:
  `POST /api/v1/auth/login|refresh|logout`.
- `authenticated`: `POST /api/v1/auth/logout-all`, `GET /api/v1/auth/me` y las
  **rutas administrativas concretas** (organizations GET/PATCH, branches,
  users, roles, permissions) — matchers explícitos por ruta, nunca
  `/api/v1/**`.
- La **autorización fina vive en method security**
  (`@EnableMethodSecurity` + `@PreAuthorize` sobre
  `AuthorizationService.hasPermission/hasBranchPermission(...)`), y la
  autoridad es **PostgreSQL** (query `EXISTS` por petición; JWT = identidad).
  Dos scopes: organización (`hasPermission`) y sucursal
  (`hasBranchPermission` = org-wide UNION membership ACTIVE de ESA branch).
  La **gestión** de memberships usa SIEMPRE permisos organization-wide
  (`branch.membership.*`): un rol branch-scoped jamás escala a la organización
  (anti privilege-escalation, probado).
  Véase `docs/04-security/rbac.md` y ADR-0005.
- **`POST /api/v1/organizations` y cualquier ruta no definida permanecen
  denegados** (`anyRequest().denyAll()`).
- Sesiones stateless, CSRF deshabilitado (API sin cookies de sesión hoy).

Autenticación (detalle: `docs/04-security/authentication.md`):

- Access JWT HS256 10m (`oauth2-resource-server`/Nimbus); se valida firma,
  expiración, issuer, audiencia, `token_type` y que `sub`/`org_id` sean UUID.
  Refresh opaco rotativo 30d, solo hash SHA-256 persistido; familia 30d
  absolutos.
- Rotación y revocación **serializadas por un lock de coordinación sobre la
  fila `users`** (`SELECT ... FOR UPDATE`; orden fijo `user` → `session`,
  PostgreSQL como autoridad); reuso de token consumido revoca la familia
  completa sin dejar sucesores vivos ante rotaciones concurrentes.
- Organización `INACTIVE` bloquea login/refresh/`/me` (errores genéricos, sin
  revelar el estado del tenant).
- **Desactivación de usuario = límite de seguridad:** `UserService` revoca
  **todas** sus refresh sessions (todas las familias) en la misma transacción
  que `status=INACTIVE` (port implementado por `auth`; lock `user → sessions`);
  reactivar no revive sesiones: el usuario vuelve a autenticarse. Los access
  JWT ya emitidos no se blacklistean (válidos hasta `exp`; `/me` resuelve la
  identidad desde la DB). Decisión pendiente para RBAC: `securityVersion`/
  security stamp o resolución autoritativa desde la DB.
- BCrypt cost 12; lockout 5 intentos → 15m; errores genéricos
  (`INVALID_CREDENTIALS`) con hash dummy anti-enumeración.
- `AUTH_JWT_SECRET` obligatoria como clave **Base64** (≥32 bytes decodificados),
  fail-fast; duraciones de tokens validadas como > 0.

No existe CORS wildcard, no existe `permitAll` global, no existen credenciales
reales en el repositorio.

Cambios de membership/roles/status de sucursal **no revocan sesiones**: la
autorización es DB-authoritative y el mismo JWT pierde/recupera acceso en la
siguiente petición (ADR-0005). Una branch `INACTIVE` deja de conceder su grant
branch-scoped de inmediato (la `EXISTS` exige `branch.status='ACTIVE'`).

**Audit Foundation** (`docs/04-security/audit.md`):

- Historial **append-only** de mutaciones administrativas; trigger `BEFORE
  UPDATE OR DELETE` en PostgreSQL (barrera final), sin API/repo de
  mutación, sin retention (pendiente por diseño).
- Actor **USER** (JWT `sub` → `actor_user_id`, same-org por FK compuesta) o
  **SYSTEM** (sin usuario técnico); `token_jti` solo correlación, nunca el
  JWT/Authorization/hash.
- `AuditPort.record` exige transacción (`MANDATORY`): el evento se escribe en
  la **misma transacción** que la mutación; si falla → rollback de la mutación.
  Operaciones fallidas → cero eventos.
- `metadata` JSONB acotada (≤16 KiB), con **rechazo defensivo** de keys
  sensibles (password, token, jwt, secret, authorization, …) case-insensitive.
- Lectura con `audit.read` **organization-wide** (un rol branch-scoped con
  `audit.read` → 403), paginada y filtrable.
- Distinción explícita: los logs `security-event=*` siguen siendo telemetría de
  operación/seguridad; `audit_events` es historial persistente de negocio.

## Futuro (no implementado)

- **Auditoría de autenticación unificada** en `audit_events` (hoy son
  `security-event` logs) — solo si aparece el requerimiento.
- **Secure Admin Bootstrap** (provisión segura del primer administrador;
  pendiente para Foundation).
- **Rate limiting** por IP (futuro, no implementado).
- **Seguridad de WebSockets** cuando ese módulo llegue (futuro, no implementado).
- **Re-autenticación e idempotencia** en operaciones monetarias (futuro).

Reporte de vulnerabilidades: `SECURITY.md`.
