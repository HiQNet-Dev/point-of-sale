# Preparación del Incremento 1I — esquema físico Auth/RBAC/Audit

**Estado: PROPUESTA VERIFICADA. Sin migraciones productivas.**

Rama: `feat/backend/rust-increment-1i-prep`, apilada sobre 1H (`b7c0f34b`).
Las propuestas siguen fuera de `migrations/local` y no se cargan desde
`pos-local`.

## 1. Esquema `installation_identity` (F1–10 §16.3)

`0002_installation_identity.proposed.sql` transcribe la ficha sin añadir campos:

| Campo | Regla física |
| --- | --- |
| `id` | UUID canónico, PK |
| `organization_id` | FK obligatoria a `organization` |
| `generation` | `INTEGER NOT NULL DEFAULT 1 CHECK (generation >= 1)` |
| `public_key_fingerprint` | `TEXT NOT NULL`, 1–128 bytes, sin NUL; **sin** `UNIQUE` |
| `status` | `TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE'))` |
| `initialized_at`, `updated_at` | UTC `...Z` obligatorios |

`UNIQUE (id, organization_id)` es superclave para la FK compuesta de `0007`.
La clave privada Ed25519 no se persiste.

Decisiones registradas como pendientes:

- **D-INST-01** catálogo completo de `status`; hoy solo `ACTIVE`.
- **D-INST-02** unicidad de `public_key_fingerprint` (global o por organización) y
  formato exacto de la huella.

## 2. Ownership del bootstrap

`0007` combina:

- `installation_id → installation_identity(id)` para que `PENDING` (con
  `organization_id` NULL) siga exigiendo una instalación real;
- `(installation_id, organization_id) → installation_identity(id, organization_id)`
  para que `COMPLETED` solo sea posible cuando instalación y organización
  pertenecen entre sí;
- FKs compuestas same-org para administrador y rol.

Probado: una instalación de Organization A no completa con Organization B; parcial
rechazado; segundo intento sin efecto; organización inexistente rechazada;
reapertura conserva el estado.

## 3. Gramática de permisos

`PermissionCode` y el `CHECK` de `permission.code` comparten lenguaje: segmentos no
vacíos, punto como separador, minúsculas ASCII, dígitos, `_` y `-`, sin comodines
ni puntos iniciales/finales/consecutivos. El seed conserva los 59 códigos
inventariados en `rust-parity-baseline.md` §4.3.

Prueba negativa cruzada: las mismas cadenas inválidas son rechazadas por Rust y por
SQLite.

## 4. D-AUTH-11 — email Unicode

| Opción | Contenido | Ventaja | Riesgo |
| --- | --- | --- | --- |
| **A (recomendada)** | Unicidad sobre valor normalizado en la aplicación (`java_trim` + `to_lowercase`); `CHECK` físico solo de forma y recorte | Compatibilidad con Java y datos existentes | Una escritura SQL directa con distinta capitalización puede crear duplicados semánticos |
| B | Restringir a ASCII imprimible y minúsculas en Rust y SQLite | Garantía física fuerte | Rompe importación de correos Unicode de Java; exige política de rechazo |

Se recomienda **A**, sin declararla aprobada. La garantía física es parcial:
SQLite `lower()` no pliega Unicode. Un `CHECK (email = lower(email))` opcional
detectaría mayúsculas ASCII, no `Ü`.

## 5. Plan definitivo de migraciones (candidato)

| Orden | Migración | Depende de |
| --- | --- | --- |
| 0001 | `0001_organization_branch.sql` (existente, intocable) | — |
| 0002 | Installation Identity | 0001 |
| 0003 | User / Credential / Session | 0001, 0002 |
| 0004 | RBAC y catálogo | 0001, 0003 |
| 0005 | Branch Memberships | 0001, 0003, 0004 |
| 0006 | Audit Event | 0001, 0003 |
| 0007 | Bootstrap Record | 0001–0004 |

SQLx versiona por prefijo numérico; no se traslada nada a `migrations/local`
hasta la aprobación.

## 6. Pruebas ejecutadas (Windows, SQLite real)

`pos-storage-sqlite/src/proposals_tests.rs` (10 pruebas):

| Prueba | Qué valida |
| --- | --- |
| `proposal_set_applies_from_empty_database_and_rejects_reapplication` | orden 0001→0007, reaplicación rechazada, reapertura |
| `installation_identity_matches_the_canonical_section` | campos, defaults, generation, FK y UUID |
| `catalog_matches_the_java_permission_list_exactly` | 59 códigos e idempotencia |
| `permission_grammar_matches_rust_validation` | paridad SQL/Rust de la gramática |
| `normalization_uniqueness_and_uuid_rules_hold` | email por organización, UUID, timestamp |
| `session_state_is_coherent_and_expiry_is_derived` | CHECK, CAS de revocación, expiración |
| `bootstrap_is_one_shot_per_installation_and_complete` | PENDING/PARCIAL/COMPLETADO, ownership, rollback |
| `rbac_catalog_resolution_preserves_java_semantics` | override, membership, estados, sin elevación |
| `audit_contract_enforces_actor_session_and_append_only` | USER/SYSTEM, metadata, triggers |
| `integrity_errors_roll_back_and_survive_reopen` | CAS obsoleto, rollback, trigger tras reapertura |

Workspace: `fmt`, `clippy -D warnings`, `test`, `build` en verde. Sin CI ni
PostgreSQL 17/Linux ejecutados en este entorno.

## 7. Paquete de aprobación

| ID | Propuesta | Clasificación | Pruebas | Estado |
| --- | --- | --- | --- | --- |
| D-AUTH-01 | tablas canónicas con `display_name` | restaura | round-trip/CHECK | Pendiente |
| D-AUTH-02 | `ACTIVE/PASSWORD_RESET_REQUIRED`; desactivación en `pos_user.status` | cambia vocabulario del laboratorio | CHECK rechaza `DISABLED` | Pendiente |
| D-AUTH-03 | `local_session.state` físico | restaura §16.45 | estado/expiración | Pendiente |
| D-AUTH-04 | sesión sin `organization_id` | restaura | FK/JOIN | Pendiente |
| D-AUTH-05 | versiones iniciales 1 | restaura | CAS | Pendiente |
| D-AUTH-06 | 59 permisos exactos | restaura | catálogo/gramática | Pendiente |
| D-AUTH-07 | `session_id` nullable; SYSTEM sin sesión | restaura | actor/sesión | Pendiente |
| D-AUTH-08 | append-only por trigger | restaura | UPDATE/DELETE | Pendiente |
| D-AUTH-09 | Argon2id comercial | — | benchmark objetivo | **Bloqueada** |
| D-AUTH-10 | bootstrap por instalación | restaura §17.3 | one-shot/ownership | Pendiente |
| D-AUTH-11 | política de email | decide brecha nueva | Unicode/ASCII | Pendiente |
| D-INST-01 | catálogo de `status` | propuesta nueva | CHECK `ACTIVE` | Pendiente |
| D-INST-02 | unicidad de huella y same-org | propuesta nueva | FK/UNIQUE | Pendiente |
| ADR-0022 | reconciliación física | documento | — | Pendiente |

Ninguna decisión está aprobada por este incremento.

## 8. Bloqueos restantes

1. Aceptación formal de D-AUTH-01..08, D-AUTH-10, D-AUTH-11 y D-INST-01/02.
2. D-AUTH-09 permanece bloqueada.
3. No se crean migraciones productivas ni adaptadores canónicos hasta esa
   aceptación.

## 9. Inicio propuesto de 1I

1. Mover `0002`–`0007` a `migrations/local/` sin tocar `0001`, promoviendo la
   suite de propuestas a prueba de migración real.
2. Sustituir `identity_lab`/`credential_lab` por adaptadores sobre las tablas
   canónicas, conservando contratos, `VerificationGate`, `Zeroizing`, CAS y
   auditoría transaccional.
3. Writer de auditoría persistente y bootstrap one-shot con guard pre-body.
4. Pruebas de integración de extremo a extremo sin red.
