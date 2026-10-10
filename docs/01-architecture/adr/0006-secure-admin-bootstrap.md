# ADR-0006: Secure Admin Bootstrap — one-shot deployment-secret provisioning

## Status

Accepted (implementado y validado por tests e2e con Testcontainers en el slice
Secure Admin Bootstrap; véase `docs/04-security/secure-bootstrap.md`).

## Context

Una instalación nueva necesita crear la primera Organization, el primer User
administrador, el primer Role administrativo y otorgar TODOS los permissions
actuales. Las restricciones que enmarcan la decisión:

- **No** existen credenciales por defecto (`default admin`, `admin123`), no hay
  seed de usuarios y **no** existe `SUPER_ADMIN`/`ROOT`/`HIQNET_ADMIN` dentro
  del POS cliente (ADR-0005, punto 5).
- `POST /api/v1/organizations` está **cerrado** (no existe el permiso
  `organization.create`): crear organizaciones es capacidad de provisioning, no
  un permiso del administrador de negocio.
- El JWT es identity-only y la autorización es database-authoritative
  (ADR-0005). El bootstrap no puede introducir bypass ni claims RBAC.
- Audit Foundation exige registrar el provisioning con actor **SYSTEM** en la
  misma transacción (append-only, `Propagation.MANDATORY`).
- La alternativa "crear un usuario administrador vía SQL/manual" no es un
  proceso reproducible, auditable ni seguro; la alternativa "abrir
  `POST /organizations` con un permiso" convertiría la provisión en una
  capacidad persistente y ampliaría la superficie.

## Decision

1. **Endpoint one-shot `POST /api/v1/bootstrap`**, gated por un secret de
   deployment (`BOOTSTRAP_SECRET`) — Base64 ≥ 32 bytes, sin default, **distinto
   de `AUTH_JWT_SECRET`** y distinto del password del administrador. Viaja
   exclusivamente en el header `X-Bootstrap-Token` y solo por HTTPS.
2. **El secret es opcional y fail-fast**: ausente/vacío → ruta cerrada
   (deny-by-default); configurado pero inválido/débil → el arranque falla.
   Tras el provisioning se recomienda eliminarlo; aunque se olvide, la DB
   impide un segundo bootstrap (defense in depth).
3. **Estado singleton en PostgreSQL** (`installation_bootstrap`, CHECK id=1 +
   CHECK todo-NULL o todo-setteado) coordinado con `SELECT ... FOR UPDATE`:
   los bootstraps concurrentes se serializan en la DB (sin lock JVM) y solo uno
   gana; el resto obtiene `409 BOOTSTRAP_ALREADY_COMPLETED`.
4. **Guardia de instalación pristina**: con estado PENDING pero organizations
   ya existentes → `409 BOOTSTRAP_NOT_ALLOWED` (nunca se "adoptan" datos
   existentes).
5. **Provisión transaccional y auditada**: Organization (+ `organization.created`),
   User, Role `administrator`, grant de TODOS los permissions leídos del
   catálogo real de PostgreSQL (nunca una segunda lista hardcodeada), Role →
   User organization-wide (`user_organization_roles`, no branch membership).
   Todo — incluyendo audit (actor SYSTEM) y estado — en UNA transacción: si
   algo falla (incluido Audit), rollback total y bootstrap reintentable.
6. **El Role `administrator` es un rol NORMAL**: sin semántica mágica, sin
   bypass por código; pierde permisos al quitarlos/desactivarse como cualquier
   rol (RBAC database-authoritative, ADR-0005).
7. **Sin tokens en la respuesta** y **sin `organization.create`**: después del
   bootstrap el administrador entra por `POST /auth/login` y la creación de
   organizations sigue siendo una capacidad de provisioning.
8. **El guard NO introduce Authentication falsa**: el HTTP pasa el filtro de
   verificación y el SecurityContext permanece vacío → audit actor = SYSTEM.

## Consequences

- Instalaciones nuevas provisionadas de forma reproducible, auditable y con
  una sola operación, sin credenciales sembradas.
- `POST /api/v1/bootstrap` solo existe mientras el secret está configurado; con
  secret ausente cae en `anyRequest().denyAll()`.
- Un caller no autenticado no puede usar el endpoint como probe de estado:
  token inválido → siempre `401 BOOTSTRAP_INVALID_TOKEN` genérico; solo con
  token válido se revela `409 BOOTSTRAP_ALREADY_COMPLETED`.
- El coste operativo: generar/almacenar/rotar un secret temporal y quitarlo
  tras la primera ejecución; documentado en
  `docs/04-security/secure-bootstrap.md`.
- Known limits: no hay recovery/backdoor para password del administrador ni
  reset API; el rate limiting del endpoint queda a cargo del gateway si la
  topología lo requiere (secret ≥ 256 bits de entropía + one-shot).