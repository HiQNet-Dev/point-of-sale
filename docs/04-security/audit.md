# Audit Foundation

**Estado:** implementado (v0.1.x Foundation, migración `V7`). Historial
**persistente, append-only** de las mutaciones administrativas y de seguridad
relevantes del POS. NO es un application/security/debug log.

## Qué es y qué no es

| | |
| --- | --- |
| **Es** historial persistente de acciones administrativas | quién hizo qué, en qué organización, sobre qué entidad, en qué branch (cuando aplica), cuándo, con qué access-JWT (`jti`) y con qué contexto seguro |
| **Logs `security-event=...`** | siguen existiendo como operación/debug/telemetría de seguridad — no se reemplazan |
| **No es** | un volcado de requests, un lago de datos, ni almacenamiento arbitrario |

## Garantía central: append-only

- La aplicación **no expone** `UPDATE`/`DELETE`/`PATCH` sobre `audit_events`
  (ni API ni repositorio: `AuditEventRepository` solo declara `save` y
  consultas).
- **PostgreSQL es la barrera final**: un trigger
  `BEFORE UPDATE OR DELETE` (`audit_events_append_only`) rechaza cualquier
  mutación directa — verificado con `JdbcTemplate` real, no mocks.
- El owner de la DB (autoridad de migración) puede alterar el schema
  deliberadamente; la aplicación ordinaria no puede mutar eventos existentes.
- **No hay purge automático ni scheduler de retención**: la política de
  retención/archivo se diseñará cuando existan volúmenes reales.

## Modelo

```
audit_events
  id              UUID      PK
  organization_id UUID      NOT NULL  → organizations (todo evento pertenece a UNA org cliente)
  branch_id       UUID      NULL      → branches(id, organization_id)  (FK compuesta: misma org)
  actor_type      USER|SYSTEM
  actor_user_id   UUID      NULL      → users(id, organization_id)     (FK compuesta: misma org)
  token_jti       VARCHAR   NULL      (jti del access JWT; nunca el JWT/header/token/hash)
  action          VARCHAR   entity.action (user.deactivated, role.permissions.replaced, …)
  entity_type     VARCHAR   organization | branch | user | role | branch_membership
  entity_id       UUID      NULL
  reason          VARCHAR   NULL      (reservado: refunds, ajustes, correcciones)
  metadata        JSONB     NULL      (contexto pequeño + estructurado, ≤ 16 KiB serializado)
  occurred_at     TIMESTAMPTZ         (UTC, Inject Clock)
```

Detalle de schema: `docs/02-database/audit-model.md`.

## Actor

- **USER** → `actor_user_id` = `sub` del access JWT (UUID). **Nunca** email como
  autoridad, ni roles/permisos en el evento. El `jti` del access JWT es
  **requerido** para un actor USER (nuestro emisor siempre lo genera; si falta,
  `AuditException` — un evento de usuario sin correlación de sesión no se
  registra).
- **SYSTEM** → `actor_user_id = NULL` (invocaciones internas: contexto de
  seguridad vacío o token anónimo/no autenticado; futuro secure bootstrap,
  mantenimiento, procesos controlados). No existen usuarios técnicos falsos.
- **Fail-closed**: un **principal autenticado que NO sea un JWT** (p. ej. un
  mecanismo de autenticación futuro o mal configurado) → `AuditException` y la
  transacción de negocio hace rollback. NUNCA se degrada silenciosamente a
  `SYSTEM` — eso destruiría la atribución y ocultaría la mala configuración.
- La resolución del actor es **central** (adaptador `AuditRecorder`): los
  services de negocio jamás importan Spring Security para auditar — simplemente
  llaman `AuditPort.record(...)` con los hechos del evento.
- Un CHECK + FKs compuestas garantizan en DB: `USER` exige `actor_user_id`, y
  ese actor pertenece a la misma organización que el evento.

## `token_jti`

Se persiste el claim `jti` del access JWT como correlación de sesión/request
(sin secretos). NUNCA se persisten: JWT completo, `Authorization`, access o
refresh token, ni token hashes.

## Atómica con la mutación

- `AuditPort.record(...)` exige transacción existente
  (`Propagation.MANDATORY`); nunca `REQUIRES_NEW`, nunca asíncrono, nunca queue.
- La escritura de auditoría ocurre en la **misma transacción** que la mutación
  de negocio: `mutation + audit = 1 commit atómico`.
- Si un AuditEvent requerido no puede persistirse → **la mutación hace
  rollback** (intencional; test explícito).
- Si la operación falla (400/401/403/404/409) → **cero eventos** de auditoría.
- Caso de seguridad fuerte: desactivación de usuario
  (`status INACTIVE` + revocación de refresh sessions + evento `user.deactivated`)
  es una única transacción atómica.

## Convenciones de naming

- `action`: `entity.action` — `organization.updated`, `branch.created`,
  `branch.updated`, `user.created`, `user.updated`, `user.deactivated`,
  `user.reactivated`, `role.created`, `role.updated`,
  `role.permissions.replaced`, `user.roles.replaced`,
  `branch.membership.created`, `branch.membership.updated`,
  `branch.membership.roles.replaced`.
- `entity_type`: nombres estables — `organization`, `branch`, `user`, `role`,
  `branch_membership`. Nunca nombres de clases Java.
- `entity_id`: UUID de la entidad principal afectada
  (p. ej. `user.updated` → entity_type=`user`, entity_id=`userId`;
  `role.permissions.replaced` → entity_type=`role`, entity_id=`roleId`).

## Metadata segura

- `metadata` JSONB, contexto pequeño (≤ **16 KiB** serializado; rechazado en
  aplicación y por CHECK en DB — se resuelve por transacción fallida, nunca se
  trunca). El límite se aplica a la **representación final sanitizada** que se
  persiste (el resultado de `asMap()` es exactamente lo que se serializa).
- PROHIBIDO dentro de metadata (rechazado **case-insensitive**, ignorando
  `_`, `-`, espacios y signos, y para compuestos evidentes): `password`,
  `passwordHash`, `token`, `accessToken`, `refreshToken`, `tokenHash`,
  `Authorization`, `jwt`, `secret`, `privateKey`, `credential`, …
- **La validación es RECURSIVA**: ninguna key sensible puede existir en
  NINGÚN nivel del árbol (nested Maps, Maps dentro de Lists, mezclas).
- **El árbol está restringido**: solo `null`, `String`, `Boolean`, `Number`,
  `Map<String, ?>` (keys String no null en todo nivel) y `Collection`;
  arrays, enums, UUID/Instant y **POJOs arbitrarios son rechazados** con
  `AuditException` (Jackson podría serializar sus propiedades hacia keys
  sensibles sin pasar por el validator).
- **Inmutabilidad profunda**: construir `AuditMetadata` copia profundamente el
  árbol en estructuras no modificables; mutar los inputs originales o lo que
  devuelve `asMap()` no cambia nada. `AuditEvent.getMetadata()` expone una
  vista profunda inmutable (también la metadata hidratada por Hibernate).
- Nunca es un volcado del request ni de DTOs completos. Ejemplos aceptables:
  `{"changedFields":["name","timezone"]}`, `{"fromStatus":"ACTIVE",
  "toStatus":"INACTIVE"}`, `{"permissionCount":4}`, `{"roleCount":2}`,
  `{"code":"manager"}`.
- Sin PII innecesaria: no se copian emails al crear usuarios (resolubles por
  id), ni IPs / user agents (la topología de proxies aún no está definida).

## Lectura

```
GET /api/v1/organizations/{organizationId}/audit-events   (audit.read, organization scope)
```

- Paginado (`PageResponse`): `?page` (0), `?size` (50, máx 100); orden fijo
  `occurredAt DESC, id DESC`. Sin sort arbitrario.
- Filtros opcionales: `branchId`, `actorUserId`, `action`, `entityType`,
  `entityId`, `from`, `to`.
- Tiempo: `from` **inclusivo**, `to` **exclusivo** (`[from, to)`); `from > to`
  → `400 VALIDATION_ERROR`.
- `branchId`/`actorUserId` foráneos (de otra organización) → `404
  BRANCH_NOT_FOUND` / `404 USER_NOT_FOUND` (nunca un filtrado silencioso a cero).
- **Solo lectura**: `POST/PATCH/PUT/DELETE /audit-events` NO existen y quedan
  cerrados por `anyRequest().denyAll()`.
- `GET /audit-events` no se audita a sí mismo (evita recursión y ruido).

## `audit.read`

- Catálogo **18 → 19**: `audit.read` (V7), autorización organization-wide.
- **CRÍTICO**: un rol asignado SOLO mediante BranchMembership que contenga
  `audit.read` **NO** puede leer el audit de la organización (contiene eventos
  de otras branches, users, roles, organization, administración de seguridad) —
  probado explícitamente (403).

## No implementado (fronteras)

- **Auditoría de autenticación** (login failures, refresh rotation, reuse,
  logout): siguen como `security-event` logs; la unificación con audit se
  evaluará solo si aparece un requerimiento.
- **Audit de lecturas** (`GET`/list/`/me`), **retention/archivo**, **device /
  terminal identity** (llegará con la fundación de hardware/cajas), **client IP**
  (topología de proxies sin definir), **partitioning / Elasticsearch /
  OpenSearch / async / outbox**.
- `organization.created` no se audita todavía: `POST /organizations` sigue
  cerrado y el provisioning real llegará con el Secure Admin Bootstrap.
- El futuro **Control Plane** tendrá su propia auditoría; este audit pertenece
  únicamente al POS cliente.