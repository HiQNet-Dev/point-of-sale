# Secure Admin Bootstrap

**Estado:** implementado (v0.1.x Foundation, migración `V8`). Provisión segura
del primer administrador SIN credenciales por defecto, sin `SUPER_ADMIN` y sin
abrir `POST /organizations`.

## Problema

Una instalación nueva necesita crear la primera Organization, el primer User
administrador, el primer Role administrativo y otorgar todos los permissions
actuales, sin romper el modelo de seguridad:

- **no** existen `default admin` / `admin123` / seed de usuarios;
- `POST /api/v1/organizations` sigue **cerrado** (no existe
  `organization.create`);
- no existe `SUPER_ADMIN` / `ROOT` / `HIQNET_ADMIN` dentro del POS.

## Decisión arquitectónica

El bootstrap es:

```text
ONE-SHOT                      (solo la primera vez; estado singleton en DB)
DEPLOYMENT-SECRET-GATED       (X-Bootstrap-Token, Base64 >= 256 bits, sin default)
DATABASE-COORDINATED          (SELECT ... FOR UPDATE sobre installation_bootstrap)
TRANSACTIONAL                 (todo en UNA transacción; fallo = rollback total)
AUDITED                       (SYSTEM, parte del audit append-only)
```

Flujo:

```text
POST /api/v1/bootstrap
  X-Bootstrap-Token: <secret>
  ├── BootstrapTokenFilter   (valida el token ANTES de leer el body; 401 genérico)
  ├── BootstrapController    (re-verifica el token: defence in depth)
  └── BootstrapService
        lock installation_bootstrap (id=1) FOR UPDATE
        completed   -> 409 BOOTSTRAP_ALREADY_COMPLETED
        orgs > 0    -> 409 BOOTSTRAP_NOT_ALLOWED   (instalación no pristina)
        create Organization                      (+ audit organization.created)
        create administrator User                (+ audit user.created)
        create Role "administrator"              (+ audit role.created)
        grant TODOS los permissions del catálogo (+ audit role.permissions.replaced)
        asignar Role al User (user_organization_roles)  (+ audit user.roles.replaced)
        marcar installation_bootstrap COMPLETED
        COMMIT
```

## Roll `administrator` NO es mágico

- Es un **Role normal** de la organización: `code = administrator`, `name =
  Administrator`, `status = ACTIVE`, sin `isAdmin`/`isSuperUser`/bypass.
- La autorización sigue siendo por **permission codes** contra PostgreSQL
  (nunca `hasRole("ADMIN")`).
- Se le conceden **todos los permissions que existen HOY** en el catálogo
  (leídos de PostgreSQL, no una segunda lista hardcodeada): hoy 19.
- Si se le quita un permiso, el acceso desaparece de inmediato (mismo JWT);
  si el rol se desactiva, deja de conceder — como cualquier rol.
- Futuras permissions de otros módulos NO se otorgan automáticamente: el
  administrador posee `role.permissions.manage` y ajustará su configuración.

## Secret deploy (`BOOTSTRAP_SECRET`)

- **Opcional**: ausente/vacío → ruta bootstrap **cerrada** (deny-by-default).
- Configurado: debe ser Base64 que decodifique a **≥ 32 bytes** (256 bits):
  `openssl rand -base64 32`. Inválido/débil → **arranque falla** (fail fast).
- Viaja SOLO en el header `X-Bootstrap-Token` (nunca query/body/cookie) y
  SOLO por HTTPS (excepto dev local).
- **Distinto de `AUTH_JWT_SECRET`** y distinto del password del administrador:
  autoriza el provisioning, no es la credencial del usuario.
- Comparación **constant-time** (digest SHA-256 + `MessageDigest.isEqual`);
  jamás se loguea, persiste o expone el secret.
- Verificado **antes** de parsear el body (filtro en la cadena de seguridad) y
  re-verificado en el controller (defense in depth). Sin Authentication falsa:
  el SecurityContext queda vacío → audit actor = SYSTEM.
- Se recomienda **eliminarlo y reiniciar** tras el provisioning; aunque se
  olvide, la DB impide un segundo bootstrap.

## Estado de instalación (V8)

```text
installation_bootstrap
  id                    SMALLINT PK   (CHECK id = 1; fila única)
  completed_at          TIMESTAMPTZ
  organization_id       UUID          -> organizations(id)
  administrator_user_id UUID          -> users(id, organization_id)      (FK compuesta)
  administrator_role_id UUID          -> roles(id, organization_id)      (FK compuesta)
```

- Fila sembrada por V8 en estado PENDING (todo NULL); CHECK exige todo-NULL o
  todo-setteado (nunca estado parcial).
- **No** almacena el secret ni hash del secret: la DB solo sabe si el bootstrap
  terminó y qué identidades creó.
- Sin `organization.create` en el catálogo: crear organizaciones sigue siendo
  una capacidad de provisioning, no un permiso de negocio.

## Concurrencia

Dos bootstraps simultáneos con el MISMO secret válido se serializan en
PostgreSQL (PESSIMISTIC_WRITE sobre la fila singleton):

```text
1 request  -> 201
1 request  -> 409 BOOTSTRAP_ALREADY_COMPLETED
1 Organization, 1 administrador, 1 rol, 1 asignación, estado COMPLETED
```

No existe lock JVM.

## Contrato

```http
POST /api/v1/bootstrap
X-Bootstrap-Token: <secret>
```

```json
{
  "organization": {
    "name": "Cafe Aurora",
    "code": "cafe-aurora",
    "timezone": "America/Mexico_City",
    "currency": "MXN",
    "locale": "es-MX"
  },
  "administrator": {
    "email": "admin@example.com",
    "password": "<password fuerte 12-128>"
  }
}
```

El cliente NO puede enviar `role`, `permissions`, `status`, `organizationId`,
`userId` (mass-assignment imposible; el servicio crea el rol y otorga TODO el
catálogo).

Respuesta `201`:

```json
{
  "organizationId": "...",
  "organizationCode": "cafe-aurora",
  "administratorUserId": "...",
  "administratorRoleId": "..."
}
```

NUNCA devuelve password, hash, secret ni tokens.

Errores:

| Código | HTTP | Cuándo |
| --- | --- | --- |
| `BOOTSTRAP_INVALID_TOKEN` | 401 | Token ausente, inválido, Base64 malformado, o bootstrap deshabilitado (siempre genérico) |
| `BOOTSTRAP_ALREADY_COMPLETED` | 409 | Ya provisionado (con token válido) |
| `BOOTSTRAP_NOT_ALLOWED` | 409 | Estado PENDING pero ya existen organizations (no se "adoptan" datos) |
| `VALIDATION_ERROR` / `WEAK_PASSWORD` / `INVALID_*` | 400 | Contrato de negocio/DTO reutilizado |

## Operación típica

```bash
export AUTH_JWT_SECRET="$(openssl rand -base64 32)"
export BOOTSTRAP_SECRET="$(openssl rand -base64 32)"
# levantar la aplicación
# POST /api/v1/bootstrap con X-Bootstrap-Token: <BOOTSTRAP_SECRET>
# 1. quitar BOOTSTRAP_SECRET
# 2. reiniciar la aplicación
# 3. login normal: POST /api/v1/auth/login { organizationCode, email, password }
```

Sin rate limiter interno: la credencial tiene ≥ 256 bits de entropía y el
endpoint es one-shot/temporal/deployment-only; rate limiting en gateway será
una defensa futura si la topología lo requiere.

## Recuperación

El password del único administrador y la destrucción accidental de sus
permissions son problemas operativos que resolverá un futuro procedimiento
soporte/recovery; NO existe backdoor ni reset API en este slice (known limit).

## Fronteras

- NO es el Control Plane HiQNet (sin customers/licenses/billing/deployments).
- NO crea `SUPER_ADMIN` ni `HIQNET_ADMIN`; NO crea branches por defecto.
- NO emite tokens (provisioning ≠ authentication).
- NO audita `login failures/refresh rotation/reuse/logout` (siguen en
  `security-event` logs).