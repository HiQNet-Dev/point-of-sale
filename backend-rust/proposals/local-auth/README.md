# Propuestas de migración Local — Auth/RBAC/Audit

**Estado: PROPUESTA. No ejecutar. No está cargada por `pos-local` ni por SQLx.**

Estos archivos viven fuera de `backend-rust/migrations/` a propósito: SQLx solo
descubre `migrations/local`, y ninguna de estas sentencias debe aplicarse mientras
D-AUTH-01..10 no tenga aceptación formal del titular.

## Orden de aplicación propuesto

| Orden | Objeto | Depende de | Estado |
| --- | --- | --- | --- |
| 0 | `0001_organization_branch.sql` (existente) | — | Publicada |
| 1 | `installation_identity` (§16.3) | — | **BLOQUEADA**: §16.3 no transcrita |
| 2 | `0002_user_credential_session.proposed.sql` | `0001` | Propuesta |
| 3 | `0003_rbac_catalog.proposed.sql` | `0002` | Propuesta |
| 4 | `0004_branch_membership.proposed.sql` | `0001`–`0003` | Propuesta |
| 5 | `0005_audit_event.proposed.sql` | `0001`–`0003` | Propuesta |
| 6 | `0006_bootstrap_record.proposed.sql` | `installation_identity`, `0001`–`0003` | Propuesta bloqueada |

`0006` no crea `installation_identity`: F1–10 §16.3 no está transcrita en este
checkout y no se inventan sus columnas. La suite de pruebas usa un sustituto
**solo de test** (`id TEXT PRIMARY KEY`) y lo marca como tal. Al aprobarse §16.3,
la migración aditiva correspondiente debe ejecutarse antes de `0006`.

## Decisiones que deben aceptarse

- **D-AUTH-01** tablas y campos canónicos, incluido `display_name`.
- **D-AUTH-02** `local_credential.state = ACTIVE/PASSWORD_RESET_REQUIRED`; la
  desactivación vive en `pos_user.status`.
- **D-AUTH-03** `local_session.state` es columna física.
- **D-AUTH-04** `local_session` sin `organization_id`; ownership por FK.
- **D-AUTH-05** `credential_version`/`session_version` iniciales en 1.
- **D-AUTH-06** catálogo exacto de 59 permisos, sembrado idempotente.
- **D-AUTH-07** `audit_event.session_id` nullable; SYSTEM no lleva sesión.
- **D-AUTH-08** append-only por trigger.
- **D-AUTH-10** `bootstrap_record` por instalación.
- **D-AUTH-11 (nuevo)** política de normalización de email frente a `lower()`
  ASCII de SQLite.
- **D-AUTH-09** parámetros Argon2id comerciales: **bloqueada**.

## Semántica de `local_session.state`

- Se crea con `ACTIVE` y `revoked_at NULL`.
- Revocar escribe en una sola sentencia `state='REVOKED'`, `revoked_at` y
  `session_version = session_version + 1` con CAS.
- La autenticación **siempre** evalúa `expires_at > now`; una fila `ACTIVE`
  vencida no autentica aunque el estado físico no se haya materializado.
- `EXPIRED` es una materialización opcional en una transición explícita, nunca
  coexiste con `revoked_at`, y no requiere tarea en segundo plano.
- `CHECK` impide `ACTIVE` con `revoked_at`, `REVOKED` sin `revoked_at` y
  `EXPIRED` con `revoked_at`.

## Normalización de email

SQLite `lower()` solo pliega ASCII. Java `String.toLowerCase(Locale.ROOT)` y Rust
`str::to_lowercase` pliegan Unicode. Por eso el `CHECK` físico exige el valor ya
recortado y con forma básica, pero **no** intenta reproducir el plegado Unicode.
La autoridad de normalización es la aplicación. D-AUTH-11 debe decidir si el
contrato Local restringe el email a ASCII o acepta que la unicidad dependa de la
normalización aplicada antes de insertar.

## Catálogo RBAC

El seed contiene los 59 códigos exactos inventariados en
`docs/05-development/rust-parity-baseline.md` §4.3. La validación Rust
(`pos_core::identity::PERMISSION_CATALOG` y `PermissionCode`) acepta el mismo
lenguaje: minúsculas, dígitos, `_` y `-` por segmento. Las pruebas comparan el
contenido SQL con la constante Rust y verifican idempotencia.

## Evidencia de validación

`pos-storage-sqlite/src/proposals_tests.rs` aplica `0001` y las propuestas sobre
una base vacía y comprueba CHECK, FKs compuestas, unicidad, estados de sesión,
catálogo, resolución org/branch, bootstrap one-shot, auditoría append-only,
rollback por integridad y reapertura. No se ejecuta desde `pos-local`.
