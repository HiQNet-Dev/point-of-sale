# Preparación del Incremento 1I — esquema físico Auth/RBAC/Audit

**Estado: PROPUESTA VERIFICADA. Sin migraciones productivas.**

Rama: `feat/backend/rust-increment-1i-prep`, apilada sobre 1H
(`b7c0f34b`). Las propuestas siguen fuera de `migrations/local` y no se cargan
desde `pos-local`.

## 1. Correcciones aplicadas

### 1.1 `bootstrap_record` por instalación (P0)

Sustituido el singleton `id INTEGER = 1` por el contrato de F1–10 §17.3:

| Campo | Tipo | Regla |
| --- | --- | --- |
| `installation_id` | TEXT UUID | PK y FK a `installation_identity(id)` |
| `state` | TEXT | `PENDING` o `COMPLETED` |
| `completed_at` | TEXT UTC | obligatorio en `COMPLETED` |
| `organization_id` | TEXT UUID | FK a `organization` |
| `administrator_user_id` | TEXT UUID | FK compuesta same-org con `pos_user` |
| `administrator_role_id` | TEXT UUID | FK compuesta same-org con `role` |
| `bootstrap_version` | INTEGER | `>= 1` |
| `updated_at` | TEXT UTC | obligatorio |

`PENDING` no admite provisioning parcial y `COMPLETED` exige todos los campos.
Una fila por instalación; un segundo intento no actualiza nada.

**Dependencia bloqueada:** `installation_identity` (§16.3) no está transcrita en
el checkout. `0006` no la crea ni inventa sus columnas; la suite usa un sustituto
solo de test.

### 1.2 Catálogo RBAC exacto (P0)

- `PermissionCode` ahora acepta `-` por segmento, igual que el catálogo Java
  (`cash-register.read`, `purchasing.order.write`).
- `PERMISSION_CATALOG` fija los 59 códigos de V5–V25.
- El seed SQL es idempotente (`ON CONFLICT DO NOTHING`) y una prueba compara
  código por código con la constante Rust.

### 1.3 Estado de sesión (P0)

- Se conserva `local_session.state` como columna física.
- Revocación atómica: `state`, `revoked_at` y `session_version` en la misma
  sentencia con CAS.
- La expiración se evalúa siempre contra `expires_at`; una fila `ACTIVE` vencida
  no autentica y no requiere materializar `EXPIRED` ni tarea en segundo plano.
- `CHECK` excluye combinaciones incongruentes.

### 1.4 Fingerprint de material (P1)

`MaterialFingerprint` ya no guarda una copia del hash: guarda un digest SHA-256 de
32 bytes y `policy_id`/parámetros. No deriva `Debug`. Se mantiene `Zeroizing`,
`VerificationGate`, la criptografía fuera de `BEGIN IMMEDIATE`, el CAS obligatorio
y el rechazo de decisiones obsoletas.

### 1.5 Validaciones físicas (P1)

- UUID canónicos en todas las tablas.
- `NOT NULL`, CHECK de catálogos y timestamps con formato UTC `...Z`.
- FK compuestas same-org, unicidad de email por organización, PK de sesión.
- `audit_event` con `session_id` nullable, SYSTEM sin sesión, metadatos ≤16 KiB,
  `event_version >= 1` y triggers append-only.
- Rollback por errores de integridad y persistencia tras reapertura.

## 2. Diferencias normativas

| Tema | Contrato | Propuesta | Clasificación |
| --- | --- | --- | --- |
| `local_session.state` | §16.45 lo exige | columna física con CHECK | **Restaura** el contrato; 1G se había apartado |
| `local_credential.state` | §16.5 `ACTIVE/PASSWORD_RESET_REQUIRED` | dos valores; desactivación en `pos_user.status` | Cambio real de vocabulario frente al laboratorio; alineado a §16.5 |
| `bootstrap_record` | §17.3 por instalación | PK `installation_id`, `bootstrap_version`, `updated_at` | Restaura el contrato; elimina el singleton Java |
| Email | Java/Rust pliegan Unicode | SQLite no puede plegar Unicode | **Brecha nueva (D-AUTH-11)** |
| RBAC | 59 códigos V5–V25 | copia exacta | Restaura; no añade códigos |

## 3. Matriz D-AUTH

| ID | Contrato canónico | Representación propuesta | Pruebas de aceptación | Riesgo | Estado |
| --- | --- | --- | --- | --- | --- |
| 01 | §16.4/16.5/16.45, `display_name` | tablas con nombres canónicos | round-trip, CHECK, reapertura | `display_name` no está en Java V3 | Pendiente |
| 02 | §16.5 `ACTIVE/PASSWORD_RESET_REQUIRED` | dos valores; sin `DISABLED` | CHECK rechaza `DISABLED` | doble fuente si se añade `DISABLED` | Pendiente |
| 03 | §16.45 `state` físico | columna + CHECK + escritura atómica | expiración derivada, revocación CAS | fila desactualizada si se omite la escritura | Pendiente |
| 04 | sesión sin `organization_id` | ownership vía `pos_user` | FK y JOIN | ninguno | Pendiente |
| 05 | versiones iniciales 1 | default 1 + CAS | carrera y conflicto | ninguno | Pendiente |
| 06 | 59 permisos | seed idéntico | comparación con `PERMISSION_CATALOG` | deriva si Java añade códigos | Pendiente |
| 07 | §17.2 `session_id` nullable | FK compuesta; SYSTEM sin sesión | inserciones válidas/inválidas | correlación perdida si se exige siempre | Pendiente |
| 08 | append-only | triggers UPDATE/DELETE | triggers sobreviven reapertura | no es inmutabilidad criptográfica | Pendiente |
| 09 | F4–19 | parámetros experimentales | benchmark hardware objetivo | alto | **Bloqueada** |
| 10 | §17.3 por instalación | `installation_id` PK/FK | one-shot, parcial, cross-org, rollback | depende de §16.3 | Pendiente (bloqueada por §16.3) |
| 11 | nuevo | email: ASCII-only o normalización solo en app | prueba de plegado Unicode | unicidad inconsistente | **Propuesta nueva** |

Ninguna decisión está aprobada.

## 4. Pruebas ejecutadas (Windows)

`pos-storage-sqlite/src/proposals_tests.rs` (8 pruebas):

| Prueba | Qué valida |
| --- | --- |
| `proposal_set_applies_from_empty_database_and_rejects_reapplication` | orden 0001→0006, reaplicación rechazada, reapertura |
| `catalog_matches_the_java_permission_list_exactly` | 59 códigos, idempotencia del seed |
| `normalization_uniqueness_and_uuid_rules_hold` | email por organización, UUID y timestamp |
| `session_state_is_coherent_and_expiry_is_derived` | CHECK de estado, CAS de revocación, expiración sin materializar |
| `bootstrap_is_one_shot_per_installation_and_complete` | PENDING/PARCIAL/COMPLETADO, cross-org, rollback por auditoría |
| `rbac_catalog_resolution_preserves_java_semantics` | override org, membership, rol/sucursal inactivos, sin elevación |
| `audit_contract_enforces_actor_session_and_append_only` | USER/SYSTEM, sesión ajena, metadata, versión, triggers |
| `integrity_errors_roll_back_and_survive_reopen` | CAS obsoleto, rollback y trigger tras reapertura |

Workspace completo: `fmt`, `clippy -D warnings`, `test`, `build` en verde
(131 pruebas). Sin CI ni PostgreSQL 17/Linux ejecutados en este entorno.

## 5. Dependencias pendientes

1. **§16.3 `installation_identity`**: bloquea la materialización de `0006`.
2. **D-AUTH-09**: parámetros Argon2id comerciales.
3. **D-AUTH-11**: política de email (ASCII o normalización en aplicación).
4. Catálogo RBAC: si Java añade códigos en V26+, el seed debe actualizarse en el
   mismo cambio contractual.

## 6. Inicio propuesto de 1I

1. Aceptar D-AUTH-01..08, 10 y decidir 11; mantener 09 bloqueada.
2. Transcribir §16.3 y añadir la migración de `installation_identity` antes de
   `0006`.
3. Mover `0002`–`0006` a `migrations/local/`, sin tocar `0001`, con la suite de
   propuestas promovida a prueba de migración real.
4. Sustituir `identity_lab`/`credential_lab` por adaptadores sobre las tablas
   canónicas, conservando sus contratos y pruebas.
5. Implementar writer de auditoría persistente y bootstrap one-shot con guard
   pre-body, sin endpoints todavía.
