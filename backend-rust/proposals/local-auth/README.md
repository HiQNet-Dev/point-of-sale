# Propuestas de migración Local — Auth/RBAC/Audit

**Estado: PROPUESTA. No ejecutar. No está cargada por `pos-local` ni por SQLx.**

Estos archivos viven fuera de `backend-rust/migrations/` a propósito: SQLx solo
descubre `migrations/local`, y ninguna de estas sentencias debe aplicarse mientras
D-AUTH-01..10 no tenga aceptación formal del titular.

Orden de aplicación propuesto:

| Archivo | Depende de |
| --- | --- |
| `0002_user_credential_session.proposed.sql` | `migrations/local/0001_organization_branch.sql` |
| `0003_rbac_catalog.proposed.sql` | `0002` |
| `0004_branch_membership.proposed.sql` | `0001`, `0002`, `0003` |
| `0005_audit_event.proposed.sql` | `0001`–`0003` |
| `0006_bootstrap_record.proposed.sql` | `0001`–`0003` |

Decisiones que deben aceptarse antes de materializarlas:

- D-AUTH-01 tablas y campos canónicos, incluido `display_name`.
- D-AUTH-02 catálogo `local_credential.state = ACTIVE/PASSWORD_RESET_REQUIRED`
  (la desactivación de usuario vive en `pos_user.status`).
- D-AUTH-03 `local_session.state` es columna física (F1–10 §16.45).
- D-AUTH-04 `local_session` sin `organization_id`; ownership por FK.
- D-AUTH-05 `credential_version`/`session_version` iniciales en 1.
- D-AUTH-06 catálogo de 59 permisos sembrado desde fuente aprobada.
- D-AUTH-07 `audit_event.session_id` nullable con FK compuesta.
- D-AUTH-08 append-only por trigger.
- D-AUTH-10 `bootstrap_record` one-shot.

D-AUTH-09 (parámetros Argon2id comerciales) permanece bloqueada y no afecta al DDL.
