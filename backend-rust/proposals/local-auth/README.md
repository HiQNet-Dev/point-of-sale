# Propuestas de migración Local — Auth/RBAC/Audit

**Estado: PROPUESTA. No ejecutar. No está cargada por `pos-local` ni por SQLx.**

Estos archivos viven fuera de `backend-rust/migrations/` a propósito: SQLx solo
descubre `migrations/local`, y ninguna de estas sentencias debe aplicarse mientras
el paquete D-AUTH/D-INST no tenga aceptación formal del titular.

## Orden definitivo propuesto

| Orden | Migración candidata | Depende de | Estado |
| --- | --- | --- | --- |
| 0001 | `0001_organization_branch.sql` (existente) | — | Publicada, intocable |
| 0002 | `0002_installation_identity.proposed.sql` | `0001` | Propuesta |
| 0003 | `0003_user_credential_session.proposed.sql` | `0001`, `0002` | Propuesta |
| 0004 | `0004_rbac_catalog.proposed.sql` | `0001`, `0003` | Propuesta |
| 0005 | `0005_branch_membership.proposed.sql` | `0001`, `0003`, `0004` | Propuesta |
| 0006 | `0006_audit_event.proposed.sql` | `0001`, `0003` | Propuesta |
| 0007 | `0007_bootstrap_record.proposed.sql` | `0001`–`0004` | Propuesta |

Los nombres son candidatos: SQLx versiona por prefijo numérico y descripción; al
aprobarse se moverán a `migrations/local/` conservando el orden. `0001` no se
modifica.

## `installation_identity` (F1–10 §16.3)

Campos: `id` UUID PK, `organization_id` FK, `generation` (default 1, `>= 1`),
`public_key_fingerprint`, `status` (inicial `ACTIVE`), `initialized_at`,
`updated_at`. La clave privada Ed25519 no se persiste en SQLite.

Decisiones pendientes:

- **D-INST-01**: catálogo completo de `status`. Solo `ACTIVE` está confirmado; el
  `CHECK` provisional solo lo admite.
- **D-INST-02**: unicidad de `public_key_fingerprint` (global o por organización)
  y formato exacto de la huella. No se añade `UNIQUE` todavía.

## Ownership del bootstrap

`0007` usa dos FKs sobre `installation_identity`:

- `installation_id → installation_identity(id)`: la instalación existe incluso en
  `PENDING` (cuando `organization_id` es NULL).
- `(installation_id, organization_id) → installation_identity(id, organization_id)`:
  cuando el bootstrap está `COMPLETED`, la organización debe ser la de la
  instalación.

Además, administrador y rol usan FKs compuestas same-org con `pos_user` y `role`.

## Gramática de permisos

Rust y SQLite aceptan el mismo lenguaje: segmentos no vacíos separados por puntos,
minúsculas ASCII, dígitos, `_` y `-`; sin comodines ni puntos iniciales, finales o
consecutivos. El seed conserva exactamente los 59 códigos de V5–V25.

## D-AUTH-11 — email Unicode

SQLite `lower()` solo pliega ASCII; Java/Rust pliegan Unicode.

**Opción A (recomendada, no aprobada).** Mantener compatibilidad Java: la
normalización (`java_trim` + `to_lowercase`) ocurre en la aplicación y la unicidad
se evalúa sobre el valor canónico. El `CHECK` físico solo exige el valor recortado
y con forma básica. Garantía frente a escrituras SQL directas: parcial; podría
añadirse un `CHECK (email = lower(email))` que detecta mayúsculas ASCII, pero no
pliega Unicode (`Ü` seguiría pasando). Riesgo: una escritura directa con distinta
capitalización puede crear duplicados semánticos.

**Opción B.** Restringir el email a ASCII imprimible y exigir minúsculas en Rust y
SQLite. Garantía fuerte, pero rompe la importación de correos Unicode de Java y
obliga a una política de rechazo.

## Evidencia de validación

`pos-storage-sqlite/src/proposals_tests.rs` (10 pruebas) aplica `0001` y las
propuestas sobre una base vacía y comprueba CHECK, FKs compuestas, unicidad,
estados de sesión, catálogo, gramática de permisos, resolución org/branch,
bootstrap one-shot y ownership, auditoría append-only, rollback por integridad y
reapertura. No se ejecuta desde `pos-local`.
