# Modelo de seguridad

**Estado actual:** autenticación implementada (`auth`, v0.1.x Foundation);
Roles/Permissions (RBAC) y autorización de negocio siguen siendo futuros.

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
- `authenticated`: `POST /api/v1/auth/logout-all`, `GET /api/v1/auth/me`.
- **Cualquier otra petición, incluida la API de negocio, sigue denegada**
  → 401/403 con JSON `ApiError` (`ApiSecurityErrorHandler`), sin detalles
  internos. La API de negocio **no abre** con un access válido hasta que
  exista RBAC.
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
- BCrypt cost 12; lockout 5 intentos → 15m; errores genéricos
  (`INVALID_CREDENTIALS`) con hash dummy anti-enumeración.
- `AUTH_JWT_SECRET` obligatoria como clave **Base64** (≥32 bytes decodificados),
  fail-fast; duraciones de tokens validadas como > 0.

No existe CORS wildcard, no existe `permitAll` global, no existen credenciales
reales en el repositorio.

## Futuro (no implementado)

- **Autorización RBAC** (módulo `users`): roles y permisos por organización.
- **Scoping efectivo** de organization/branch en cada endpoint (autorización
  por pertenencia, no solo deny).
- **Rate limiting** por IP (futuro, no implementado).
- **Seguridad de WebSockets** cuando ese módulo llegue (futuro, no implementado).
- **Re-autenticación e idempotencia** en operaciones monetarias (futuro).

Reporte de vulnerabilidades: `SECURITY.md`.
