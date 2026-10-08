# ADR-0004: Estrategia de acceso y refresh tokens

## Status

Accepted

## Context

HiQNet POS es una API de negocio deny-by-default. Cuando llega autenticación
hay que decidir cómo representar una sesión de cliente: un JWT de larga
duración es cómodo pero difícil de revocar; un refresh stateless carece de
rotación y detección de robo. El sistema también exige (spec de
autenticación): no reutilizar el JWT expirado como refresh, revocación
efectiva al cerrar sesión, y detección de reuso del refresh token.

## Decision

1. **Access token = JWT corto (10 minutos).** Firmado HS256 con
   `spring-boot-starter-oauth2-resource-server` / Nimbus (sin librerías
   extra). Claims: `sub` (userId), `org_id`, `token_type=access`, `jti`,
   `iss`, `aud`, `iat`, `exp`. El decoder de cada petición valida firma,
   expiración, issuer, audiencia y `token_type` (un token tipo refresh jamás
   es aceptado como access).
2. **Refresh token = opaco, 256 bits, rotativo.** Generado con `SecureRandom`,
   Base64 URL-safe sin padding. Solo se persiste su hash SHA-256 (`token_hash`
   único en `refresh_sessions`) — el valor crudo jamás toca la base de datos.
3. **Rotación atómica anti-reuso.** Cada refresh marca consumida la sesión
   (`rotated_at` + `replaced_by_session_id`) y crea una nueva en la misma
   familia, bajo `SELECT ... FOR UPDATE` (`PESSIMISTIC_WRITE`). Presentar un
   token ya rotado/revocado **revoca toda la familia** y genera warning de
   seguridad: si alguien roba el token, el atacante y la víctima se invalidan
   mutuamente.
4. **TTLs configurables.** Access 10m, refresh 30d, familia 30d **absolutos**:
   la rotación nunca extiende `family_expires_at` (sesión no se auto-renueva
   para siempre).
5. **Contraseñas BCrypt (cost 12, test 10)** y lockout temporal por intentos
   fallidos (5 → 15m) configurables. Errores de login genéricos
   (`INVALID_CREDENTIALS`) con hash dummy para igualar el tiempo de respuesta
   y evitar enumeración de usuarios.
6. **Secret fail-fast.** `AUTH_JWT_SECRET` (≥32 bytes) **sin default**: si el
   entorno no la define, la aplicación no arranca. El único valor de test vive
   en `src/test/resources`.
7. **Logout.** `logout` revoca la sesión actual (idempotente, nunca revela si
   el token existía); `logout-all` revoca todas las refrescables del usuario
   (no invalida access ya emitidos: son cortos).

## Consequences

- Revocación real vía refresh: un `logout` deja al token de refresh inútil en
  segundos; el access expira en ≤10m.
- Detección de robo: reuso de token consumido mata la familia completa.
- Los access JWT siguen siendo verificables offline por otros servicios
  internos (firma compartida), pero el negocio que requiera revocación
  inmediata debe mirar la sesión.
- `AUTH_JWT_SECRET` es obligatoria en todos los entornos excepto tests;
  producción exige 256 bits de entropía real (p. ej. `openssl rand -base64 32`).
- El módulo `auth` no introduce roles/permisos: la API de negocio continúa
  deny-by-default hasta el módulo `users` (RBAC).