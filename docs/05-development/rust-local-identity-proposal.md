# Propuesta técnica Local — identidad, sesión y dependencias de auditoría

**Estado: PROPUESTA TÉCNICA, SIN DDL NI APROBACIÓN NORMATIVA.**

Este documento acompaña a `pos-core::identity` y no modifica F1–10..13, F4–14,
F4–18..21, ADR-0021 ni CC-0021. Las tablas fixture de los tests no son parte del
root `migrations/local`.

## Alcance y frontera

La composición futura debe seguir esta dirección:

```text
handle/credencial verificada por adaptador
  -> LocalSession + PosUser vigentes y scoped
  -> permisos resueltos desde DB actual
  -> VerifiedPrincipal (no construible desde DTO)
  -> autorización del caso de uso
  -> AuditEventRecord { actor USER, session_id }
  -> misma UoW SQLx y commit
```

`SYSTEM` no se obtiene de una sesión ni de un UUID enviado por cliente. Solo un
flujo interno de `pos-core` con procedencia cerrada puede construirlo. La
verificación criptográfica, contraseña cruda y logging quedan fuera del dominio.

## Matriz de `pos_user` candidata

| Campo | Tipo lógico / Rust | SQLite candidata | NULL/default | Integridad | Regla | Fuente | Estado / gate |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `id` | `EntityId` | `TEXT` UUID | NN | PK; `UNIQUE(id, organization_id)` | estable, generado por aplicación | Java V3/V5; F1–10 por confirmar | Propuesta / DDL13 |
| `organization_id` | `EntityId` | `TEXT` | NN | FK `organization(id)` | owner inmutable | Java V3; F1–10 §16.4 por confirmar | Propuesta / DDL13 |
| `display_name` | `DisplayName` | `TEXT` | NN o decisión pendiente | límite UTF-16 en core | requerido por alcance Local 1F; **no existe en Java V3** | Requisito 1F | Pendiente de ficha canónica |
| `email` | `NormalizedEmail` | `TEXT` | NN | `UNIQUE(organization_id,email)`; no vacío | trim Java + lowercase, nunca global | Java V3/V4 | Propuesta / PAR identidad |
| `status` | `PosUserStatus` | `TEXT` | NN / `ACTIVE` candidato | `IN ('ACTIVE','INACTIVE')` | INACTIVE no autentica ni autoriza | Java V3 | Propuesta / AS identidad |
| `version` | `u64` | `INTEGER` | NN / `0` | `>=0`; CAS | revisión optimista | Java V3 | Propuesta / CAS |
| `created_at`,`updated_at` | reloj UTC | `TEXT` RFC 3339 candidato | NN | formato/clock a aprobar | no tiempo de cliente | Java V3 | Pendiente físico |

## Matriz de `local_credential` candidata

| Campo | Tipo lógico / Rust | SQLite candidata | NULL/default | Integridad | Regla | Fuente | Estado / gate |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `id` | `EntityId` | `TEXT` UUID | NN | PK | identidad de credencial | F1–10 §16.45 por confirmar | Propuesta |
| `user_id`,`organization_id` | `EntityId` | `TEXT` | NN | FK compuesta hacia `pos_user` | evita credential cross-org | Java V3/V5 como antecedente | Bloqueado por `pos_user` |
| `kind` | `CredentialKind` | `TEXT` | NN | `BCRYPT_LEGACY` / `ARGON2ID` candidato | no algoritmo implícito | F4–19; auth lab | Pendiente aprobación |
| `hash_reference` | `CredentialReference` | `TEXT` | NN | longitud/forma por algoritmo | nunca serializar/loggear; no plaintext | Java V3; F4–19 | Pendiente benchmark/DDL |
| `policy_id` | `String` cerrada | `TEXT` | NN | catálogo de policies | separa algoritmo de parámetros | F4–19 auth lab | Pendiente aprobación |
| `state` | `CredentialState` | `TEXT` | NN / `READY` candidato | catálogo cerrado | `RESET_REQUIRED` no hace fallback | F4–19 auth lab | Propuesta |
| `version` | `u64` | `INTEGER` | NN / 0 | CAS | upgrade/revocation sin overwrite | F4–19 auth lab | Propuesta / CAS |

## Matriz de `local_session` candidata

| Campo | Tipo lógico / Rust | SQLite candidata | NULL/default | Integridad | Regla | Fuente | Estado / gate |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `id` | `EntityId` | `TEXT` UUID | NN | PK | referencia opaca de sesión para auditoría | F1–10 §16.46–50 por confirmar | Propuesta |
| `organization_id`,`user_id` | `EntityId` | `TEXT` | NN | FK compuesta a `pos_user` | owner derivado, no escogido por cliente | Java V3/V5 | Bloqueado por `pos_user` |
| `handle_hash` | `SessionHandleHash` | `TEXT` | NN | UNIQUE, 64 hex SHA-256 candidata | nunca guardar el handle crudo | Java `refresh_sessions` | Propuesta; no importa sesiones Java |
| `state` / `revoked_at` | derivado + `Option<UtcSeconds>` | `TEXT`/UTC candidato | `revoked_at` NULL | activo si no revocada y no expirada | `REVOKED` gana a `EXPIRED` | Requisito 1F; Java antecedente | Pendiente ficha |
| `created_at`,`expires_at` | `UtcSeconds` lógico | UTC `TEXT` candidato | NN | `expires_at > created_at` | validación offline | Java V3 | Pendiente físico |
| `version` | `u64` | `INTEGER` | NN / 0 | CAS | revocación efectiva bajo carrera | Requisito 1F | Propuesta / CAS |

La sesión Local sustituye conceptualmente `token_jti` para correlación Local. No
es JWT ni refresh token Java y no se importan handles/sesiones históricas.

## `audit_event`, bootstrap y RBAC

La ficha de [`audit_event`](rust-local-audit-event-proposal.md) es la autoridad
interna de esta propuesta. En 1F se corrigieron los nombres al crosswalk canónico:
`session_id`, `metadata_json` y `event_version INTEGER NOT NULL DEFAULT 1`
candidato. `event_version` versiona el **contrato serializado** del hecho, no la
revisión CAS de una fila; los eventos existentes no se reinterpretan al cambiar
una versión futura.

| Relación / tabla | Dependencia mínima | Integridad requerida | Estado |
| --- | --- | --- | --- |
| `audit_event.actor_user_id` | `pos_user` | FK compuesta same-org; USER exige actor, SYSTEM lo prohíbe | Pendiente DDL |
| `audit_event.session_id` | `local_session` | ownership compatible con actor/org; nulabilidad USER pendiente de decisión | Pendiente DDL/Auth |
| `bootstrap_record` | Organization, PosUser, Role | singleton one-shot, PENDING o COMPLETED completo, FKs same-org | F1–10 §17.3 por confirmar; Java V8 antecedente |
| `role`, `permission`, `user_organization_role` | `pos_user` | permiso vigente DB-authoritative, sin wildcard/cache | Java V5 antecedente; Local pendiente |
| `branch_membership`, `branch_membership_role` | PosUser, Branch, Role | FKs compuestas same-org y branch scope no eleva a org | Java V6 antecedente; Local pendiente |

## Bootstrap seguro y transición de credenciales

El flujo Java V8 se conserva como contrato, no como endpoint abierto: lock del
registro singleton, instalación prístina, Organization, administrador, rol normal
`administrator`, grants desde catálogo, asignación org-wide, auditoría SYSTEM y
un solo commit. Rust 1F solo identifica `bootstrap_record` y no implementa
secreto pre-body, provisioning ni writer productivo.

BCrypt legado puede verificarse/migrarse a Argon2id únicamente tras login válido,
con parámetros explícitos y benchmark aprobado. El `migration-lab` ya mantiene
vectores sintéticos `AUTH-MIG-01..08`; `pos-core` solo modela `CredentialKind`,
`CredentialState` y referencias redactadas. No hay migración de hashes, rehash
productivo ni importador de sesiones en 1F.

## Decisiones cerradas y abiertas

**Implementado/verificable:** ownership inmutable de usuario/sesión, email
normalizado, estado ACTIVE/INACTIVE, CAS de usuario/sesión, sesión expirada o
revocada fail-closed, principal con permisos obtenidos por puerto DB-authoritative
y actor USER construido solo desde ese principal.

**Pendiente de aprobación física/normativa:** todas las tablas Local, nombre y
nulabilidad final de `display_name`, timestamps físicos, catálogo de credential
policy, constraints de `session_id`, versión/retención de eventos, bootstrap
record y relaciones RBAC Local. D01–D07 no reemplazan esa aprobación.
