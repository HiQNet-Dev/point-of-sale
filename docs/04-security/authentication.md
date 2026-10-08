# Autenticación (módulo `auth`)

**Estado:** implementado (v0.1.x Foundation). Documenta el comportamiento
real; decisiones de diseño en `docs/01-architecture/adr/0004-access-and-refresh-token-strategy.md`.

## Flujo

```text
POST /api/v1/auth/login  → 200 { accessToken, refreshToken, ... }
   access: JWT HS256, 10m (token_type=access, sub=userId, org_id, iss, aud, jti)
   refresh: opaco 256 bits; solo su SHA-256 se persiste en refresh_sessions

POST /api/v1/auth/refresh  { refreshToken } → 200 (nueva pareja; rotación)
   - la sesión presentada queda consumida (rotated_at, replaced_by_session_id)
   - reuso de una sesión consumida/revocada → revoca TODA la familia (401 INVALID_REFRESH_TOKEN)
   - la familia expira a los 30d ABSOLUTOS: rotar no la extiende

POST /api/v1/auth/logout     { refreshToken } → 204 (idempotente, token desconocido = 204)
POST /api/v1/auth/logout-all (Bearer access)   → 204 (revoca TODAS las refresh del usuario)
GET  /api/v1/auth/me         (Bearer access)   → 200 { userId, organizationId, email, status }
```

`/me` resuelve la identidad desde la **base de datos**, nunca desde las
claims del JWT: sirve también para detectar usuarios desactivados antes de
que expire su access.

## Configuración (`app.auth`, sobrescribible por variable de entorno)

| Propiedad | Variable | Default | Notas |
| --- | --- | --- | --- |
| `jwt-secret` | `AUTH_JWT_SECRET` | **sin default (fail-fast)** | ≥32 bytes (256 bits) |
| `issuer` | — | `hiqnet-pos` | claim `iss` |
| `audience` | — | `hiqnet-pos-api` | claim `aud` |
| `access-token-ttl` | — | `10m` | vigencia del access JWT |
| `refresh-token-ttl` | — | `30d` | vigencia de cada sesión refresh |
| `family-ttl` | — | `30d` | vigencia absoluta de la familia (no se extiende) |
| `max-failed-attempts` | — | `5` | antes del lockout temporal |
| `lockout-duration` | — | `15m` | duración del lockout |
| `password-strength` | — | `12` | cost BCrypt (test: 10) |

Solo el perfil `test` contiene un secreto de prueba; `dev`/`prod` exigen
`AUTH_JWT_SECRET` real:

```bash
openssl rand -base64 32
```

## Postura de seguridad (`SecurityConfig`)

- `permitAll`: `POST /api/v1/auth/login|refresh|logout`,
  `/actuator/health`, `/error` y docs **si** `app.security.permit-opendoc`.
- `authenticated`: `POST /api/v1/auth/logout-all`, `GET /api/v1/auth/me`.
- **Todo lo demás sigue denegado** (`denyAll`): la API de negocio
  (organizations/branches) no abre aunque se presente un access válido hasta
  que exista RBAC (`users`). Verificado por tests de integración.

## Errores

| Código | HTTP | Cuándo |
| --- | --- | --- |
| `INVALID_CREDENTIALS` | 401 | Login fallido (genérico: org, email o password) |
| `INVALID_REFRESH_TOKEN` | 401 | Refresh desconocido, consumido, revocado o de familia muerta |
| `AUTHENTICATION_REQUIRED` | 401 | Sin token de acceso |
| `TOKEN_INVALID` | 401 | Access rechazado (firma, issuer, audiencia, tipo) |
| `TOKEN_EXPIRED` | 401 | Access expirado |
| `FORBIDDEN` | 403 | Denegado (incluye APIs de negocio hasta RBAC) |

## Protecciones aplicadas

- **Anti-enumeración:** 401 genérico para org/email/password incorrectos;
  hash dummy ejecutado cuando el usuario no existe (timing uniforme).
- **Normalización:** email trim + lowercase previo al lookup
  (`lower(email) = lower(:email)`); único por organización (no global).
- **Nunca se persiste** la contraseña ni el refresh crudo (solo BCrypt y
  SHA-256 respectivamente).
- **Lockout temporal** (5 intentos → 15m) por contraseña fallida; el contador
  se suma incluso cuando la respuesta es 401 (commit intencional del
  registro de intento).
- **Reuso detectado** revoca la familia completa y queda en logs de seguridad
  (`security-event=refresh-reuse`); ninguna respuesta distingue reuse de
  token desconocido.
- **Logs de seguridad estructurados** (`security-event=...`) con ids, nunca
  tokens/contraseñas/Authorization.
- Rate limiting por IP (edge/gateway) queda fuera de scope de esta fundación;
  documentado como pendiente para el endurecimiento.

## Límites conocidos (pendientes, no bloquean la fundación)

- **Purga de sesiones caducadas:** las filas de `refresh_sessions` vencidas
  (30d) no se eliminan automáticamente; los TTLs se aplican en lectura y la
  tabla crece con hashes muertos. Pendiente: job de limpieza programado
  (`expires_at < now()` y rotadas/revocadas más viejas que el TTL de familia).
- **Ventana de carrera en revocación masiva:** `logout-all` /
  revocación por reuso corren como `UPDATE` por familia/usuario; una rotación
  *en vuelo* (que ya insertó su sesión sucesora) puede no verse incluida en
  el mismo instante, dejando una sesión viva hasta el TTL de familia.
  Probabilidad baja (doble conflicto simultáneo); mitigación futura con
  una marca de revocación por familia validada en cada rotación.