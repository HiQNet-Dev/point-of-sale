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

## Adaptador

`pos-storage-sqlite::identity_lab` expone `SqliteIdentityLab`, ligado a una
`&mut SqliteConnection`, con:

- `session_by_hash`: deriva `organization_id` mediante `JOIN pos_user`; no existe
  columna de organización en la sesión.
- `user_by_id`: búsqueda scoped por organización y usuario.
- `effective_permissions`: alcance de organización o unión organización +
  membership activa; exige sucursal activa de la misma organización.
- `revoke_session_cas`: `UPDATE ... WHERE id = ? AND session_version = ? AND
  revoked_at IS NULL`, clasificando después `Updated`, `Missing` o `Conflict`.

Tablas de laboratorio (fixtures, `STRICT`):

| Tabla | Invariantes comprobadas |
| --- | --- |
| `identity_user_lab` | `UNIQUE(id, organization_id)`, `UNIQUE(organization_id, email)` |
| `identity_session_lab` | `UNIQUE(session_hash)`, `expires_at > created_at`, `session_version >= 1`, FK a usuario |
| `identity_role_lab` | `UNIQUE(id, organization_id)` |
| `identity_role_permission_lab` | PK `(role_id, permission_code)` |
| `identity_user_role_lab` | FKs compuestas same-org usuario/rol |
| `identity_membership_lab` | `UNIQUE(organization_id, branch_id, user_id)`, `UNIQUE(id, organization_id)`, FKs compuestas branch/usuario |
| `identity_membership_role_lab` | PK `(organization_id, membership_id, role_id)`, FKs compuestas |

## Evidencia ejecutada (Windows, SQLite real)

| Prueba | Qué demuestra |
| --- | --- |
| `lab_adapter_builds_scoped_principals_and_denies_other_scopes` | scope A1 autoriza `branch.update`; A2, org y otra organización fallan |
| `lab_adapter_fails_closed_for_inactive_state_and_bad_sessions` | membership inactiva, sucursal inactiva, sesión revocada/expirada y usuario inactivo |
| `lab_session_revocation_is_cas_and_single_writer_per_version` | dos pools, un solo `Updated`, versión final `2`, sesión inexistente = `Missing` |
| `lab_authorization_re_resolves_inside_the_unit_of_work_and_rolls_back` | `BEGIN IMMEDIATE`: membership desactivada dentro de la UoW deniega; rollback restaura; revocación revertida |
| `lab_schema_enforces_cross_organization_ownership` | FK compuesta rechaza membership cross-org; organización derivada de la sesión |

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
