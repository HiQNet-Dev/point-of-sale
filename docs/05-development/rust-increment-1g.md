# Incremento 1G — adaptador SQLite experimental de identidad y permisos

**Estado: LABORATORIO.** Se implementa una composición DB-authoritative real sobre
SQLite con tablas `identity_*_lab`. No hay DDL productivo, ni auth, ni RBAC, ni
endpoints. Las tablas de este incremento **no** pertenecen a `migrations/local` y
no se ejecutan desde `pos-local`.

## Cambios de frontera

Los puertos de identidad pasaron de síncronos a asíncronos con un único origen:

```text
LocalIdentityRepository + PermissionAuthority
        └── una sola fuente (LocalPrincipalService<S>)
```

Motivos:

1. Un adaptador SQLx real no puede implementar métodos síncronos sin `block_on`
   ni conexiones clonadas.
2. Identidad y permisos deben leerse de la misma conexión para que la decisión no
   mezcle estados distintos.
3. `authenticate` recibe `&mut self`, así que el mismo servicio puede construirse
   sobre la conexión de una UoW activa.

`pos-core` sigue sin dependencias de runtime: solo los tests usan `tokio`.

## Corrección de paridad RBAC (1G.1)

La primera versión del adaptador exigía membership activa para cualquier scope de
sucursal. Eso divergía de `AuthorizationService` de Java, donde el permiso de
organización es un **override** para operaciones de sucursal. Se corrigió:

| Regla Java | Implementación Rust |
| --- | --- |
| `hasPermission` exige usuario, organización y rol ACTIVOS | `resolve_permissions(org, None)` con esos `JOIN` |
| Override: permiso global concede la operación de sucursal sin membership | la unión incluye siempre las grants organizacionales |
| `hasBranchPermission` exige branch ACTIVA, membership ACTIVA, rol ACTIVO | `BRANCH_PERMISSIONS_SQL` |
| `hasBranchPermissionIgnoringBranchStatus` no exige branch ACTIVA | `BRANCH_PERMISSIONS_RECOVERY_SQL` + `PrincipalScope::recovery` |
| Sucursal ajena o inexistente → 404 en el servicio, no en autorización | no aporta permisos y no revela existencia |
| Rol de sucursal nunca se eleva a organización | el scope de organización usa solo `branch_id = None` |

La recuperación es explícita: `ScopeMode::Recovery` forma parte del
`PrincipalScope`, de modo que un principal de recuperación no autoriza
operaciones normales y uno normal no autoriza recuperación.

## Revocación de sesión scoped

`revoke_session_cas` recibe un `VerifiedPrincipal`; el `user_id` y la
`organization_id` salen del principal, no de la petición. La sentencia incluye
ambos en el `WHERE` y la clasificación posterior solo consulta sesiones del mismo
owner. Consecuencias verificadas:

- sesión propia y versión vigente → `Updated`;
- repetición o versión obsoleta → `Conflict`;
- sesión de otro usuario de la misma organización → `Missing`;
- sesión de otra organización o UUID inexistente → `Missing`, sin distinguirlos.

## Adaptador

`pos-storage-sqlite::identity_lab` expone `SqliteIdentityLab`, ligado a una
`&mut SqliteConnection`, con:

- `session_by_hash`: deriva `organization_id` mediante `JOIN pos_user`.
- `user_by_id`: búsqueda scoped por organización y usuario.
- `effective_permissions`: override de organización + pierna de sucursal normal.
- `effective_permissions_for_recovery`: idéntico sin exigir sucursal activa.
- `revoke_session_cas`: CAS scoped por principal.

Tablas de laboratorio (fixtures, `STRICT`):

| Tabla | Invariantes comprobadas |
| --- | --- |
| `identity_user_lab` | `UNIQUE(id, organization_id)`, `UNIQUE(organization_id, email)` |
| `identity_session_lab` | `UNIQUE(session_hash)`, `expires_at > created_at`, `session_version >= 1`, FK a usuario |
| `identity_role_lab` | `UNIQUE(id, organization_id)`, `status IN ('ACTIVE','INACTIVE')` |
| `identity_role_permission_lab` | PK `(role_id, permission_code)` |
| `identity_user_role_lab` | FKs compuestas same-org usuario/rol |
| `identity_membership_lab` | `UNIQUE(organization_id, branch_id, user_id)`, `UNIQUE(id, organization_id)`, FKs compuestas branch/usuario |
| `identity_membership_role_lab` | PK `(organization_id, membership_id, role_id)`, FKs compuestas |

## Evidencia ejecutada (Windows, SQLite real)

| Prueba | Qué demuestra |
| --- | --- |
| `java_parity_organization_override_grants_branch_without_membership` | override global, incluso sin membership y con sucursal inexistente |
| `java_parity_branch_grant_does_not_promote_to_organization_scope` | sin elevación de sucursal a organización |
| `java_parity_unknown_or_foreign_branch_grants_nothing_without_error` | sucursal ajena o sin membership no concede ni filtra |
| `java_parity_inactive_role_organization_and_membership_revoke_grants` | estados ACTIVOS exigidos |
| `java_parity_recovery_authorizes_inactive_branch_but_normal_does_not` | recuperación explícita y no reutilizable |
| `java_parity_inactive_or_foreign_states_fail_closed` | usuario inactivo, sesión revocada o expirada |
| `java_parity_concurrent_permission_changes_are_seen_on_reauthentication` | cambios de rol observados al re-autenticar |
| `java_parity_storage_failure_during_authorization_fails_closed` | error SQL → `ScopeDenied` |
| `session_revocation_is_scoped_to_the_authenticated_principal` | revocación propia, ajena y de otro tenant |
| `session_revocation_and_authorization_share_the_unit_of_work` | `BEGIN IMMEDIATE`, denegación y rollback |
| `lab_schema_enforces_cross_organization_ownership` | FKs compuestas y organización derivada |

Las pruebas 1B–1F (Organization, Branch, UoW, cancelación, rollback, auditoría
fixture, identidad pura) siguen ejecutándose sin cambios.

## Lo que este incremento **no** demuestra

- No hay `pos_user`, `local_credential`, `local_session` ni tablas RBAC de
  producto: el DDL sigue pendiente de aprobación.
- No hay verificación criptográfica, login, emisión de handles, refresh ni
  revocación de familias.
- No hay RBAC completo: solo el puerto y su consulta de laboratorio.
- No hay writer de auditoría persistente; la auditoría sigue siendo fixture.
- No hay endpoints Axum ni OpenAPI Rust.
- El Migration Lab PostgreSQL 17 en Linux/WSL2 no se ejecutó aquí.

## Riesgos

1. El adaptador de laboratorio y el futuro de producto comparten contratos, pero
   no esquema: cualquier divergencia deberá resolverse con la ficha aprobada.
2. La consistencia depende de que el llamador construya el servicio sobre la
   conexión de la UoW; un uso sobre el pool fuera de la transacción vuelve a ser
   una decisión puntual.
3. `session_version` funciona como CAS sin columna separada; si la ficha final
   distingue versión y revisión, el adaptador deberá ajustarse.
