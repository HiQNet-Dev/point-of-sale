# Propuesta técnica Local — identidad, sesión, RBAC y dependencias de auditoría

**Estado: PROPUESTA TÉCNICA, SIN DDL NI APROBACIÓN NORMATIVA.**

Este documento acompaña a `pos-core::identity` y no modifica F1–10..13, F4–14,
F4–18..21, ADR-0021 ni CC-0021. Las tablas fixture de los tests no son parte del
root `migrations/local` y no se ejecutan en producto.

## Alcance y frontera

La composición futura mantiene esta dirección:

```text
handle/credencial verificada por adaptador
  -> LocalSession + PosUser vigentes y scoped
  -> permisos resueltos desde DB actual para el scope exacto
  -> VerifiedPrincipal (no construible desde DTO)
  -> authorize(permiso, scope) deny-by-default
  -> mutación + AuditEventRecord { actor USER, session_id }
  -> misma UoW SQLx y commit
```

`SYSTEM` no se obtiene de una sesión ni de un UUID enviado por cliente. Solo un
flujo interno de `pos-core` con procedencia cerrada puede construirlo. La
verificación criptográfica, contraseña cruda y logging quedan fuera del dominio.

## Crosswalk Java → Rust → F1–10 → SQLite candidato

| Concepto | Java vigente | Rust 1F | F1–10 | SQLite candidato | Gate |
| --- | --- | --- | --- | --- | --- |
| Usuario | `users` V3/V4 | `PosUser` | §16.4 `pos_user` | ver matriz | DDL13 / PAR identidad |
| Credencial | columnas en `users` V3 | `LocalCredential` | §16.5 `local_credential` | ver matriz | DDL13 |
| Sesión | `refresh_sessions` V3 | `LocalSession` | §16.45 `local_session` | ver matriz | DDL13 / Auth |
| Roles | `roles`, `role_permissions`, `user_organization_roles` V5 | puerto `PermissionAuthority` | §16.46–49 RBAC Local | ver matriz | RBAC |
| Membership | `branch_memberships`, `branch_membership_roles` V6 | scope de `PrincipalScope` | §16.46–49 | ver matriz | RBAC branch |
| Auditoría | `audit_events` V7 | `AuditEventRecord` | §17.2 `audit_event` | `rust-local-audit-event-proposal.md` | DDL13 |
| Bootstrap | `installation_bootstrap` V8 | contrato documentado | §17.3 `bootstrap_record` | ver matriz | Bootstrap |

## Matriz de `pos_user` candidata

| Campo | Tipo lógico / Rust | SQLite candidata | NULL/default | Integridad | Regla | Fuente | Estado |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `id` | `EntityId` | `TEXT` UUID | NN | PK; `UNIQUE(id, organization_id)` | estable | Java V3/V5 | Propuesta |
| `organization_id` | `EntityId` | `TEXT` | NN | FK `organization(id)` | owner inmutable | Java V3 | Propuesta |
| `display_name` | `DisplayName` | `TEXT` | NN o decisión | límite UTF-16 en core | requerido por 1F; **no existe en Java V3** | Requisito 1F | Pendiente ficha |
| `email` | `NormalizedEmail` | `TEXT` | NN | `UNIQUE(organization_id,email)`; `email = lower(trim(email))` | trim Java + lowercase | Java V3/V4 | Propuesta / PAR |
| `status` | `PosUserStatus` | `TEXT` | NN / `ACTIVE` | `IN ('ACTIVE','INACTIVE')` | INACTIVE no autentica | Java V3 | Propuesta / AS |
| `version` | `u64` | `INTEGER` | NN / `0` | `>= 0`; CAS | revisión optimista Java | Java V3 | Propuesta / CAS |
| `created_at`,`updated_at` | reloj UTC | `TEXT` RFC 3339 | NN | formato/clock a aprobar | no tiempo de cliente | Java V3 | Pendiente físico |

## Matriz de `local_credential` candidata

Java no tiene tabla de credenciales: `users.password_hash`, `users.version`,
`users.failed_login_attempts` y `users.locked_until` cubren el caso con un único
algoritmo. F1–10 §16.5 separa finalidad, algoritmo, parámetros y bloqueo.

| Campo | Tipo lógico / Rust | SQLite candidata | NULL/default | Integridad | Regla | Fuente | Estado |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `id` | `EntityId` | `TEXT` UUID | NN | PK | identidad de credencial | F1–10 §16.5 por confirmar | Propuesta |
| `user_id`,`organization_id` | `EntityId` | `TEXT` | NN | FK compuesta a `pos_user` | evita credential cross-org | Java V3/V5 antecedente | Bloqueado por `pos_user` |
| `credential_kind` | `CredentialKind` (`PASSWORD`/`PIN`) | `TEXT` | NN | `IN ('PASSWORD','PIN')` | finalidad, no algoritmo | F1–10 §16.5 | Pendiente catálogo |
| `algorithm` | `CredentialAlgorithm` (`LEGACY_BCRYPT`/`ARGON2ID`) | `TEXT` | NN | `IN ('LEGACY_BCRYPT','ARGON2ID')` | no algoritmo implícito | F1–10 §16.5; F4–19 | Pendiente aprobación |
| `secret_hash` | `SecretHash` (redactado) | `TEXT` | NN | longitud ≤255; nunca plaintext | solo hash codificado | F1–10 §16.5; Java V3 | Pendiente benchmark |
| `params_json` | `CredentialParameters` | `TEXT` JSON | NN | `json_valid`; ≤2048 bytes; claves no sensibles | parámetros públicos de verificación | F1–10 §16.5; F4–19 | Propuesta |
| `credential_version` | `u64` (inicia `1`) | `INTEGER` | NN / `1` | `>= 1`; CAS | versión del material | F1–10 §16.5 | Pendiente default |
| `policy_id` | `String` cerrada | `TEXT` | NN | catálogo de policies | separa algoritmo de parámetros | F4–19 auth lab | Pendiente aprobación |
| `state` | `CredentialState` | `TEXT` | NN | catálogo cerrado a aprobar | `RESET_REQUIRED`/`DISABLED` son estados de dominio; Java usa `users.status` | F1–10 §16.5; Java V3 | **Pendiente ficha** |
| `failed_login_attempts` | `u32` | `INTEGER` | NN / `0` | `>= 0` | ventana temporal, nunca bloqueo permanente | Java V3; `User.recordFailedLogin` | Propuesta |
| `locked_until` | `Option<UtcSeconds>` | `TEXT` UTC | NULL | ventana temporal | `locked_until > now` implica bloqueo | Java V3 | Propuesta |
| `created_at`,`updated_at` | `UtcSeconds` lógico | `TEXT` UTC | NN | formato a aprobar | reloj inyectado | Java V3 | Pendiente físico |

## Matriz de `local_session` candidata

| Campo | Tipo lógico / Rust | SQLite candidata | NULL/default | Integridad | Regla | Fuente | Estado |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `id` | `EntityId` | `TEXT` UUID | NN | PK | referencia para `audit_event.session_id` | F1–10 §16.45 | Propuesta |
| `user_id` | `EntityId` | `TEXT` | NN | FK a `pos_user` | owner derivado, no elegido por cliente | F1–10 §16.45 | Bloqueado por `pos_user` |
| `session_hash` | `SessionHash` | `TEXT` | NN | UNIQUE, 64 hex SHA-256 | nunca guardar el handle crudo | Java `refresh_sessions.token_hash` | Propuesta |
| `state` | derivado (`LocalSessionState`) | `TEXT` candidato | decisión | catálogo a aprobar | revocación gana a expiración | F1–10 §16.45 | **Nulabilidad/representación pendiente** |
| `created_at`,`expires_at` | `UtcSeconds` lógico | `TEXT` UTC | NN | `expires_at > created_at` | validación offline | F1–10 §16.45 | Pendiente físico |
| `revoked_at` | `Option<UtcSeconds>` | `TEXT` UTC | NULL | — | una vez revocada no revive | F1–10 §16.45 | Propuesta |
| `session_version` | `u64` (inicia `1`) | `INTEGER` | NN / `1` | `>= 1`; CAS | versión de sesión | F1–10 §16.45 | Propuesta |

**Sin columna `organization_id`.** F1–10 §16.45 no la incluye: el ownership se
deriva de `pos_user` mediante FK. El dominio conserva `organization_id` como
ownership de agregado y el adaptador debe obtenerlo del usuario y verificar que
coincide; nunca lo acepta de la petición.

La sesión Local sustituye conceptualmente `token_jti` para correlación Local. No
es JWT ni refresh token Java y no se importan handles ni sesiones históricas.

## Matriz RBAC candidata

El catálogo canónico está fijado en `pos_core::identity::PERMISSION_CATALOG`
(59 códigos de V5–V25) y `PermissionCode` acepta el mismo lenguaje que SQLite:
minúsculas, dígitos, `_` y `-` por segmento (`cash-register.read`).

| Tabla | Dependencias | Integridad requerida | Fuente | Estado |
| --- | --- | --- | --- | --- |
| `permission` | — | catálogo sembrado por migración; sin CRUD cliente | Java V5 | Propuesta |
| `role` | `organization` | `UNIQUE(organization_id, code)`, `UNIQUE(id, organization_id)`, status cerrado | Java V5 | Propuesta |
| `role_permission` | `role`, `permission` | PK `(role_id, permission_code)` | Java V5 | Propuesta |
| `user_organization_role` | `pos_user`, `role` | FKs compuestas same-org; PK `(organization_id,user_id,role_id)` | Java V5 | Bloqueado por `pos_user` |
| `branch_membership` | `pos_user`, `branch` | `UNIQUE(organization_id,branch_id,user_id)`; status cerrado; FKs compuestas | Java V6 | Bloqueado por `pos_user` |
| `branch_membership_role` | `branch_membership`, `role` | PK `(organization_id,membership_id,role_id)`; FKs compuestas | Java V6 | Bloqueado |

## `audit_event` y `bootstrap_record`

La ficha de [`audit_event`](rust-local-audit-event-proposal.md) conserva los
nombres canónicos `session_id`, `metadata_json` y `event_version`. En 1F no se
introducen cambios de esquema; el estado sigue siendo propuesta.

`bootstrap_record` se mantiene como dependencia física de `pos_user`/`role`:
singleton one-shot, PENDING o COMPLETED completo y FKs compuestas same-org. Java
V8 (`installation_bootstrap`) es el antecedente funcional; Rust no implementa
provisioning, secreto pre-body ni writer productivo.

## Autorización contextual y paridad RBAC

`VerifiedPrincipal` conserva el `PrincipalScope` exacto (organización, sucursal y
modo) con el que se resolvieron sus permisos. La decisión se toma siempre con
`authorize(permission, scope)`:

1. Scope distinto del principal → `ScopeMismatch`, sin consultar permisos.
2. Permiso ausente en ese scope → `PermissionDenied`.
3. Cualquier duda de sesión, usuario o autoridad → error cerrado antes de mutar.

Contrato de `PermissionAuthority`, alineado con
`AuthorizationService.hasPermission/hasBranchPermission` de Java:

| Caso Java | Rust |
| --- | --- |
| `hasPermission(org, p)` | `resolve_permissions(user, org, None)` |
| `hasBranchPermission(org, branch, p)` | `resolve_permissions(user, org, Some(branch))` |
| `hasBranchPermissionIgnoringBranchStatus` | `resolve_permissions_for_recovery(...)` + `authenticate_recovery` |

Reglas verificadas:

- **Override de organización**: las grants organizacionales se incluyen siempre
  en un scope de sucursal; no se exige membership de esa sucursal.
- **Pierna de sucursal**: requiere sucursal ACTIVA (normal) o existente en la
  organización (recuperación), membership ACTIVA, usuario ACTIVO, rol ACTIVO y
  organización ACTIVA.
- **Sin elevación**: una grant de membership nunca aparece en el scope de
  organización, que se resuelve con `branch_id = None`.
- **Sin fuga de tenant**: una sucursal ajena o inexistente no produce error ni
  revela existencia; simplemente no aporta permisos.
- **Recuperación explícita**: el principal de recuperación solo autoriza scopes
  en modo recuperación; no habilita operaciones normales ni al contrario.
- **Revocación de sesión**: la operación parte de un `VerifiedPrincipal`; la
  organización y el usuario proceden del principal, no de la petición.

Consistencia: `authenticate` es una decisión puntual. Para mutaciones críticas la
identidad, sesión, membership, estado de sucursal y permisos deben resolverse
dentro de la misma unidad de trabajo que la mutación (`BEGIN IMMEDIATE` + RAII de
SQLx), sin cachés de permisos como autoridad de escritura. Los puertos son
asíncronos y comparten una única fuente (`LocalPrincipalService<S>`), y el
incremento 1G implementa esa composición con tablas de laboratorio.

## Verificación criptográfica y transición

`pos-credentials` implementa la frontera real de verificación (1H): BCrypt legado,
Argon2id con políticas explícitas, detección de rehash, gates de entrada y handles
opacos de sesión. La transición BCrypt → Argon2id se ejecuta en
`pos-storage-sqlite::credential_lab` sobre la conexión de la UoW y usa CAS sobre
`credential_version`; un fallo de auditoría revierte el cambio de material.

Los parámetros (`lab-argon2-v0`, `lab-argon2-v1`) son experimentales. El paquete de
decisiones D-AUTH-01..10 y el orden de migraciones propuesto están en
[`rust-increment-1h1.md`](rust-increment-1h1.md); ninguna migración se crea sin esa
aceptación. El 1H.1 corrige además que `local_session` sí incluye `state` físico
en F1–10 §16.45: derivarlo en el dominio no elimina la columna.

## Decisiones abiertas

**Aprobables de inmediato:** nombres canónicos de `local_credential` y
`local_session`; unicidad y normalización de email; bloqueo temporal por
`locked_until`; scope contextual y deny-by-default; ausencia de `organization_id`
física en `local_session`.

**Dependen del contrato de auth Local:** catálogo físico de `state` en credencial
y sesión; nulabilidad de `session_id` en auditoría para USER; obligatoriedad de
`params_json`; default de `credential_version`/`session_version`; retención;
tabla de sesiones y su lifecycle.

**Bloqueadas por tablas no aceptadas:** cualquier DDL de `pos_user`,
`local_credential`, `local_session`, RBAC, `audit_event` o `bootstrap_record`.
D01–D07 no sustituyen esta aprobación.
