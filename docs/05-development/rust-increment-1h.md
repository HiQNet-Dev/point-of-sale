# Incremento 1H — verificación de credenciales Local y transición BCrypt → Argon2id

> **Actualización 1H.1.** El CAS de `persist` ya no se ignora y la criptografía se
> ejecuta fuera de la transacción de escritura. Ver
> [`rust-increment-1h1.md`](rust-increment-1h1.md). El diseño de `verify_and_upgrade`
> descrito abajo quedó sustituido por `snapshot → prepare → apply`.

**Estado: LABORATORIO CRIPTOGRÁFICO.** Existe verificación real y transición con CAS
sobre SQLite de laboratorio. **No** hay login, sesiones productivas, entrega de
handles, DDL aprobado ni endpoints. Todas las políticas son experimentales.

## Crate `pos-credentials`

Frontera independiente de SQLx, Axum y transporte:

| Elemento | Responsabilidad |
| --- | --- |
| `VerificationOutcome` | `Verified { rehash_required }`, `Rejected`, `Unusable` |
| `VerifyRequest` | finalidad, algoritmo declarado, hash almacenado y secreto presentado |
| `verify_credential` | verificación BCrypt/Argon2id con gates previos |
| `hash_password` | Argon2id nuevo con sal CSPRNG de 16 bytes |
| `Argon2Policy` | `lab-argon2-v0` (origen) y `lab-argon2-v1` (destino), parámetros explícitos |
| `SessionHandle` | 32 bytes de `OsRng`, `Debug` redactado y `hash()` SHA-256 |

Reglas implementadas:

- El algoritmo procede de la fila, nunca se deduce del prefijo del hash.
- BCrypt: variantes `$2a/$2b/$2y`, coste `4..=12`, longitud exacta 60, entrada
  ≤72 bytes sin truncado; errores de la biblioteca se colapsan sin incluir el hash.
- Argon2id: PHC ≤256 bytes, `v=19`, salida de 32 bytes, sal de 16 bytes y
  coincidencia **exacta** de `m/t/p` con una política registrada.
- Entrada vacía, con NUL o fuera de límite se rechaza antes de invocar al
  verificador.
- `Unusable` no prueba un fallback: el caso de uso debe exigir reset.
- Ningún `Debug` público muestra contraseña, hash ni handle.

## Transición transaccional

`pos-storage-sqlite::credential_lab` implementa:

```text
BEGIN IMMEDIATE (UoW existente)
  -> cargar credencial del principal
  -> estado ACTIVO y ventana de bloqueo
  -> verificar BCrypt/Argon2id
  -> (si procede) hash Argon2id + CAS credential_version
  -> auditoría del llamador en la MISMA conexión
  -> COMMIT único
```

`CredentialTransition` distingue `Verified { rehash_performed }`, `Rejected`,
`Locked` y `ResetRequired` para el caso de uso interno; una futura API debe
colapsarlos en un error genérico para no facilitar enumeración.

Comprobado en SQLite real:

| Prueba | Resultado |
| --- | --- |
| `legacy_bcrypt_login_upgrades_to_argon2id_in_one_commit` | hash Argon2id, versión 2, auditoría presente |
| `wrong_password_locks_after_policy_and_never_upgrades` | 3 fallos → lock, sin rehash; ventana vencida migra |
| `cas_conflict_leaves_the_stored_material_untouched` | CAS obsoleto = `Conflict` sin sobrescritura |
| `audit_failure_rolls_back_the_credential_upgrade` | rollback deja BCrypt y versión 1 |
| `corrupt_hash_requires_reset_without_fallback_or_further_mutation` | `RESET_REQUIRED` idempotente |
| `disabled_and_locked_credentials_reject_before_expensive_verification` | sin verificación costosa |
| `scoped_verification_only_touches_the_authenticated_principal` | usuario y organización desde el principal |
| `session_handle_stores_only_its_hash` | nunca se persiste el handle claro |

## Handles de sesión

El laboratorio genera 32 bytes con `OsRng`, valida 64 hex minúsculas y persiste solo
el SHA-256. No se implementa entrega al cliente, rotación, familias ni refresh.
`SessionHandle::as_client_secret` está documentado como uso exclusivo del
composition root y nunca de logs.

## Benchmark observado

Ejecución local con `cargo run -q -p pos-credentials --example argon2_bench`:

```text
os=windows arch=x86_64 cpus=12 rounds=5
policy=lab-argon2-v0 m_kib=8192  t=1 p=1 hash_ms=83.1  verify_ms=82.9  status=OBSERVED
policy=lab-argon2-v1 m_kib=19456 t=2 p=1 hash_ms=400.2 verify_ms=401.8 status=OBSERVED
```

No es una aprobación: falta benchmark en el hardware objetivo, verificación de
interoperabilidad con el entorno Java y aceptación formal de parámetros (F4–19).

## Paquete de decisiones Auth/RBAC/Audit

Aprobación acotada que no toca las decisiones financieras de ADR-0021:

| ID | Decisión propuesta | Estado |
| --- | --- | --- |
| D-AUTH-01 | Crear `pos_user`, `local_credential` y `local_session` con los nombres canónicos de F1–10 §16.4/16.5/16.45, incluido `display_name` | Pendiente |
| D-AUTH-02 | Catálogo `state` de credencial `ACTIVE/RESET_REQUIRED/DISABLED`; la desactivación de usuario sigue en `pos_user.status` | Pendiente |
| D-AUTH-03 | Estado de sesión derivado de `revoked_at`/`expires_at` en lugar de columna `state` | Pendiente |
| D-AUTH-04 | `local_session` sin columna `organization_id`; ownership vía FK a `pos_user` | Pendiente |
| D-AUTH-05 | `credential_version` y `session_version` inician en `1` y actúan como CAS | Pendiente |
| D-AUTH-06 | Catálogo RBAC `permission` sembrado por migración; rol administrador ordinario | Pendiente |
| D-AUTH-07 | `audit_event.session_id` nullable con FK compuesta; obligatoriedad por acción a definir | Pendiente |
| D-AUTH-08 | Append-only por trigger SQLite; no se declara inmutabilidad criptográfica | Pendiente |
| D-AUTH-09 | Parámetros Argon2id comerciales solo tras benchmark en hardware objetivo | Bloqueado |
| D-AUTH-10 | Bootstrap one-shot con secreto en configuración de despliegue, nunca en DB | Pendiente |

## Plan de migraciones aditivas propuesto (no ejecutado)

| Orden | Archivo propuesto | Depende de |
| --- | --- | --- |
| 1 | `0002_user_credential_session.sql` | `0001_organization_branch.sql` |
| 2 | `0003_rbac_catalog.sql` | `pos_user`, `organization` |
| 3 | `0004_branch_membership.sql` | `branch`, `pos_user`, `role` |
| 4 | `0005_audit_event.sql` | `pos_user`, `local_session`, `branch` |
| 5 | `0006_bootstrap_record.sql` | `organization`, `pos_user`, `role` |

Ninguna se crea en este incremento: requieren aceptación de D-AUTH-01..10 y no se
edita la migración publicada `0001`.

## Límites

- Sin login, sin emisión de sesiones, sin refresh ni revocación de familias.
- Sin RBAC productivo ni auditoría persistente.
- Sin endpoints Axum/OpenAPI.
- Migration Lab PostgreSQL 17 no ejecutado en este entorno.
- `PAR-08` sigue siendo el único gate histórico aprobado.
