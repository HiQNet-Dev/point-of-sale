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
   - revoca la FAMILIA de la sesión presentada (cadena de rotación de ese
     dispositivo/login), no solo la fila; seguro ante una rotación concurrente
POST /api/v1/auth/logout-all (Bearer access)   → 204 (revoca TODAS las refresh del usuario)
GET  /api/v1/auth/me         (Bearer access)   → 200 { userId, organizationId, email, status }
   - identidad siempre desde la BASE DE DATOS, nunca desde las claims del JWT
   - exige user ACTIVE y organización ACTIVE
```

`/me` resuelve la identidad desde la **base de datos**, nunca desde las
claims del JWT: sirve también para detectar usuarios/organizaciones
desactivados antes de que expire el access.

`refreshExpiresAt` es **`min(expires_at, family_expires_at)`**: el instante en
que el refresh deja de ser utilizable. Con TTLs distintos el contrato sigue
siendo inequívoco.

## Configuración (`app.auth`, sobrescribible por variable de entorno)

| Propiedad | Variable | Default | Notas |
| --- | --- | --- | --- |
| `jwt-secret` | `AUTH_JWT_SECRET` | **sin default (fail-fast)** | **Base64**; al decodificar debe dar **≥32 bytes (256 bits)** |
| `issuer` | — | `hiqnet-pos` | claim `iss` |
| `audience` | — | `hiqnet-pos-api` | claim `aud` |
| `access-token-ttl` | — | `10m` | vigencia del access JWT; **> 0** (fail-fast) |
| `refresh-token-ttl` | — | `30d` | vigencia de cada sesión refresh; **> 0** (fail-fast) |
| `family-ttl` | — | `30d` | vigencia absoluta de la familia (no se extiende); **> 0** |
| `max-failed-attempts` | — | `5` | antes del lockout temporal |
| `lockout-duration` | — | `15m` | duración del lockout; **> 0** (fail-fast) |
| `password-strength` | — | `12` | cost BCrypt (test: 10) |

`AUTH_JWT_SECRET` se interpreta como **clave codificada en Base64** (no texto
plano): se decodifica y debe aportar ≥32 bytes reales, de lo contrario la
aplicación **no arranca**. No existe default ni fallback. Generarla con:

```bash
openssl rand -base64 32
```

Las duraciones (`access-token-ttl`, `refresh-token-ttl`, `family-ttl`,
`lockout-duration`) se validan como **estrictamente positivas** en el arranque
(`@DurationMin`); un `0` o un valor negativo falla el arranque en vez de
degradar silenciosamente. Solo el perfil `test` contiene un secreto de prueba.

## Postura de seguridad (`SecurityConfig`)

- `permitAll`: `POST /api/v1/auth/login|refresh|logout`,
  `/actuator/health`, `/error` y docs **si** `app.security.permit-opendoc`.
- `authenticated`: `POST /api/v1/auth/logout-all`, `GET /api/v1/auth/me`.
- **Todo lo demás sigue denegado** (`denyAll`): la API de negocio
  (organizations/branches) no abre aunque se presente un access válido hasta
  que exista RBAC (`users`). Verificado por tests de integración.
- Los fallos de **bearer token** (firma, expiración, issuer, audiencia, tipo,
  claims) los responde el `ApiSecurityErrorHandler` de la propia cadena de
  resource-server, de modo que el contrato JSON `TOKEN_INVALID`/`TOKEN_EXPIRED`
  se cumple siempre (no un 401 con cuerpo vacío).

## Validación del access JWT

El decoder (`NimbusJwtDecoder` + validadores) valida, en cada petición:

```text
firma (HS256)  +  expiración (exp/nbf)  +  issuer  +  audiencia
token_type = "access"  +  sub y org_id presentes y con formato UUID
```

`sub` y `org_id` deben ser **UUID válidos**. Un token correctamente firmado
pero con `sub = "abc"` (o `org_id` inválido/ausente) se rechaza aquí como
`401 TOKEN_INVALID`, nunca llega al controller ni provoca un `500` en
`UUID.fromString(...)`.

## Concurrencia y locking (revocación vs rotación)

Todas las mutaciones de sesiones de un mismo usuario se serializan con un
**lock de coordinación sobre la fila `users`** (`SELECT ... FOR UPDATE`),
adquirido SIEMPRE antes de cualquier fila de `refresh_sessions`
**orden de locking fijo: `user` → `session`**). Participan:
`refresh`, `logout`, `logout-all` y la revocación por familia (reuse).

PostgreSQL es la autoridad (nada de `synchronized`/locks JVM), por lo que la
garantía se mantiene con múltiples instancias. Invariantes cubiertas por
tests de carrera reales (Testcontainers + `ExecutorService`/`CountDownLatch`,
con verificación del estado final en base de datos):

| Carrera | Resultado |
| --- | --- |
| refresh vs refresh | máximo un refresh exitoso; nunca dos sucesores válidos |
| refresh vs logout | nunca queda una sesión nueva viva detrás del logout |
| refresh vs logout-all | tras `logout-all` no queda ninguna refresh válida |
| reuse (revocación de familia) vs refresh | la familia queda sin sesiones vivas |

## Errores

| Código | HTTP | Cuándo |
| --- | --- | --- |
| `INVALID_CREDENTIALS` | 401 | Login fallido (genérico: org inexistente/inactiva, email o password) |
| `INVALID_REFRESH_TOKEN` | 401 | Refresh desconocido, consumido, revocado, de familia muerta o de organización inactiva |
| `AUTHENTICATION_REQUIRED` | 401 | Sin token de acceso |
| `TOKEN_INVALID` | 401 | Access rechazado (firma, issuer, audiencia, tipo, `sub`/`org_id`) o identidad no operativa (`/me`) |
| `TOKEN_EXPIRED` | 401 | Access expirado |
| `FORBIDDEN` | 403 | Denegado (incluye APIs de negocio hasta RBAC) |

## Estado de la organización (tenant)

- **Login:** una organización `INACTIVE` se trata igual que una inexistente:
  sin lookup de usuario, hash dummy y `401 INVALID_CREDENTIALS` genérico. No se
  revela el estado del tenant.
- **Refresh:** se revoca la familia y se responde `401 INVALID_REFRESH_TOKEN`.
- **`/me`:** se responde `401 TOKEN_INVALID` (la identidad ya no es operativa).

## Desactivación de usuarios (administración de Users)

La desactivación administrativa (`UserService`, PATCH status=INACTIVE) revoca
**todas** las refresh sessions del usuario (todas las familias/dispositivos) en
la **misma transacción** que el cambio de status, vía el port de aplicación
`UserSessionRevocationPort` implementado por `auth` (orden de locking
`user → sessions`). Reactivar **no** revive sesiones revocadas ni emite tokens:
el usuario debe autenticarse de nuevo (login nuevo).

**Access JWT y reactivación:** la desactivación **no** blacklistea JWTs ya
emitidos — un access token sigue siendo criptográficamente válido hasta su
`exp` (10m); `/me` consulta User/Organization desde la DB y rechaza identidades
no operativas, y las business APIs continúan deny-by-default. Antes de abrir
RBAC revisaremos si hace falta un `securityVersion`/security stamp o si la
resolución autoritativa desde la DB es suficiente (decisión pendiente, fuera de
esta fundación).

## Protecciones aplicadas

- **Anti-enumeración:** 401 genérico para org/email/password incorrectos (o
  tenant inactivo); hash dummy ejecutado cuando el usuario no existe (timing
  uniforme).
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
