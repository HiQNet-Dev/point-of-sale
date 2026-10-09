# Audit — modelo de datos

**Implementado en `V7__create_audit_events.sql`** (Audit Foundation). Guía de
seguridad/alimentación: `docs/04-security/audit.md`.

## `audit_events`

| Columna | Tipo | Reglas |
| --- | --- | --- |
| `id` | `UUID` PK | Generado en aplicación |
| `organization_id` | `UUID` NOT NULL | FK → `organizations(id)` — todo evento pertenece a una organización cliente |
| `branch_id` | `UUID` NULL | FK compuesta → `branches(id, organization_id)` (V6) — misma org garantizada |
| `actor_type` | `VARCHAR(16)` NOT NULL | CHECK `IN ('USER','SYSTEM')` |
| `actor_user_id` | `UUID` NULL | FK compuesta → `users(id, organization_id)` (V5) — misma org garantizada |
| `token_jti` | `VARCHAR(64)` NULL | `jti` del access JWT (correlación; nunca el JWT/header/hash) |
| `action` | `VARCHAR(80)` NOT NULL | `entity.action` (p. ej. `user.deactivated`) |
| `entity_type` | `VARCHAR(40)` NOT NULL | `organization` / `branch` / `user` / `role` / `branch_membership` |
| `entity_id` | `UUID` NULL | UUID de la entidad principal afectada |
| `reason` | `VARCHAR(255)` NULL | Reservado (refunds, ajustes, correcciones) |
| `metadata` | `JSONB` NULL | Contexto estructurado pequeño; ≤ 16 KiB serializados |
| `occurred_at` | `TIMESTAMPTZ` NOT NULL | UTC, `Instant`, Clock inyectable |

## Constraints de integridad

| Invariante | Mecanismo |
| --- | --- |
| **Append-only** | Trigger `BEFORE UPDATE OR DELETE` (`audit_events_append_only`) que rechaza con `check_violation`; la API/repositorio no exponen mutación |
| `USER` exige actor | CHECK `(actor_type='USER' AND actor_user_id IS NOT NULL) OR (actor_type='SYSTEM' AND actor_user_id IS NULL)` |
| Actor de la misma org | FK compuesta `(actor_user_id, organization_id) → users(id, organization_id)` (MATCH SIMPLE: `NULL` no impone nada) |
| Branch de la misma org | FK compuesta `(branch_id, organization_id) → branches(id, organization_id)` |
| Metadata acotada | CHECK `metadata IS NULL OR octet_length(metadata::text) <= 16384` (+ validación en aplicación) |

## Índices

Cada uno responde a una consulta real del endpoint de lectura (siempre anclado
por `organization_id` y ordenado `occurred_at DESC, id DESC`):

- `(organization_id, occurred_at DESC, id DESC)` — listado principal.
- `(organization_id, branch_id, occurred_at DESC)` — filtro `branchId`.
- `(organization_id, actor_user_id, occurred_at DESC)` — filtro `actorUserId`.
- `(organization_id, entity_type, entity_id, occurred_at DESC)` — filtros
  `entityType`/`entityId`.

Sin partitioning, sin Timescale/ES (escala inicial), sin índices especulativos.

## Dirección de módulos

- `shared.audit` (contrato): `AuditPort`, `AuditRecord`, `AuditMetadata`,
  `AuditException` — los módulos de negocio dependen SOLO de esto.
- `audit` (implementación): `domain` (entidad inmutable + repositorio
  restringido), `infrastructure` (`AuditRecorder`: actor central, Clock,
  MANDATORY, límite de metadata), `application` (`AuditQueryService`), `api`
  (lectura). Compilación: `audit → shared.audit` y negocio → `shared.audit`;
  sin ciclos.