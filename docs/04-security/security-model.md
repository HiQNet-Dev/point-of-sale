# Modelo de seguridad

**Estado actual:** solo la configuración base de foundation existe. No hay
autenticación, usuarios, roles ni autorización de negocio (futuro).

## Principios

1. **Deny by default.** Todo lo no explícitamente permitido está denegado.
2. **Least privilege.** Cada rol/entidad ve y hace lo mínimo necesario.
3. **Autorización server-side.** El frontend **nunca** es fuente de verdad de
   permisos; solo UX. Cualquier regla crítica se valida en el servidor.
4. **Scoping organization/branch.** Toda consulta de negocio se acota al
   contexto de la organización y, cuando aplica, a la sucursal.
   **Sin fallbacks `branchId=1`, sin IDs mágicos, sin "por defecto la primera sucursal".**
5. **Secretos por entorno.** Contraseñas y credenciales viven en variables de
   entorno por perfil; nunca en Git. Valores de ejemplo en dev
   (`hiqnet_dev` / `hiqnet_dev_only`) son **solo locales no productivos**.
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

- `permitAll` **solo** en `/actuator/health` y `/error`.
- Cualquier otra petición: **denegada** → 401/403 con JSON `ApiError`
  (`ApiSecurityErrorHandler`), sin detalles internos.
- Sesiones stateless, CSRF deshabilitado (API sin cookies de sesión hoy).

No existe CORS wildcard, no existe `permitAll` global, no existen credenciales
reales en el repositorio.

## Futuro (no implementado)

- **Autenticación** (módulo `auth`): login, sesiones/tokens, logout.
- **Autorización RBAC** (módulo `users`): roles y permisos por organización.
- **Scoping efectivo** de organization/branch en cada endpoint.
- **Rate limiting** (futuro, no implementado).
- **Seguridad de WebSockets** cuando ese módulo llegue (futuro, no implementado).
- **Re-autenticación e idempotencia** en operaciones monetarias (futuro).

Reporte de vulnerabilidades: `SECURITY.md`.
