# Incremento 1N — Preparación final de migraciones productivas y primera integración SQLite

**Estado: SQL CANDIDATO VERIFICADO + WRITER DE AUDITORÍA IMPLEMENTADO (NO PRODUCTIVO).**

Rama: `feat/backend/rust-migrations-1n`, basada en
`feat/backend/rust-increment-1i-prep` (`f41ad54e4f5893eb0ca807708cbec9764bdfddfe`).
No se modificó `dev`, `main`, Flyway V1–V25 ni
`migrations/local/0001_organization_branch.sql`.

## 1. Preflight

| Elemento | Estado observado |
| --- | --- |
| Base | `feat/backend/rust-increment-1i-prep` @ `f41ad54e` |
| Ramas apiladas | 1J `9859041d`, 1K `0e91e0fc`, 1L `39d1b773`, 1M `0ce5bac9` (PRs #68–#71) |
| `dev` / `main` | `329c0984` / `c290f518`, intactos |
| Worktree | Limpio al iniciar; sin cambios ajenos |
| Propuestas | `proposals/local-auth/0002`–`0007`, fuera de `migrations/local` |
| Linux/WSL2 + PostgreSQL 17 | No disponibles en este entorno |

## 2. Revisión de las migraciones candidatas

Se revisaron las seis propuestas contra F1–10 y el expediente 1M. Resultado por
migración:

| Migración | Nombres y tipos | Nulabilidad / defaults | CHECK | FK compuestas | Índices | CAS | Resultado |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `0002` installation_identity | Conforme §16.3 | Conforme | `status` solo `ACTIVE` (D-INST-01) | Superclave `(id, organization_id)` presente | n/a | n/a | Sin cambios |
| `0003` user/credential/session | Conforme §16.4/16.5/16.45 | Conforme | Estados alineados | `local_credential` y `local_session` cuelgan de `pos_user` | Presentes | `*_version` inicial 1 | Sin cambios |
| `0004` rbac_catalog | 59 códigos | Conforme | Gramática de permiso | n/a | Presentes | `version` | Sin cambios |
| `0005` branch_membership | Conforme | Conforme | Estados | Same-org en tres niveles | Presentes | `version` | Sin cambios |
| `0006` audit_event | Conforme §17.2 | Conforme | **Corregido** (ver 2.1) | Branch, actor y sesión | 4 índices + 2 triggers | `event_version` | Corregido |
| `0007` bootstrap_record | Conforme §17.3 | Conforme | `PENDING`/`COMPLETED` | Doble FK sobre instalación y same-org de administrador/rol | n/a | `bootstrap_version` | Sin cambios |

### 2.1 Error físico encontrado y corregido

`audit_event.occurred_at` exigía estrictamente `YYYY-MM-DDTHH:MM:SSZ`, pero el
contrato de dominio `pos_core::audit::AuditOccurredAt` admite fracción opcional
(`YYYY-MM-DDTHH:MM:SS[.fracción]Z`). Cualquier evento con milisegundos habría sido
rechazado por el `CHECK`.

Corrección aplicada en `0006_audit_event.proposed.sql`: el `CHECK` acepta las dos
formas y sigue rechazando texto libre. Prueba:
`audit_occurred_at_accepts_optional_fraction_and_rejects_junk` (4 formas válidas,
6 inválidas).

No se alteró ningún otro `CHECK` ni se añadieron columnas.

## 3. Writer de auditoría

`pos-storage-sqlite::audit_writer` implementa la inserción canónica:

| Garantía | Mecanismo |
| --- | --- |
| Append-only | Se apoya en los triggers de `0006`; el writer solo ejecuta `INSERT` |
| USER/SYSTEM | `actor_type` y `actor_user_id` provienen de `AuditActor`, construible solo dentro de `pos-core` |
| Sesión del actor | Se escribe desde el principal; la FK `(session_id, actor_user_id) → local_session(id, user_id)` la verifica |
| Scope | Organización y sucursal salen del principal o del evento de bootstrap |
| Metadata | JSON canónico ya validado (≤16 KiB) o `NULL`; nunca se trunca |
| Atomicidad | Único parámetro es una `SqliteTransaction`; el commit es del caso de uso |
| Rollback | Un error devuelve `ApplicationError::Audit` y el llamador revierte |

Vías de construcción añadidas a `pos-core::audit`:

- `AuditEventRecord::bootstrap_completed(..)`: única vía pública a un actor
  `SYSTEM`, limitada a `SecureBootstrap`.
- `AuditEventRecord::from_verified_principal(..)`: actor, organización, sucursal
  y sesión salen del principal verificado; el transporte no elige nada.

Pruebas (6): bootstrap SYSTEM persistido y reabierto, append-only tras reapertura,
rollback sin fila, rechazo de SYSTEM con sesión, evento USER desde principal con
metadata canónica y fracción de segundo, y `event_version`/metadata sin
truncamiento.

**No productivo**: la tabla vive en `proposals/`, `pos-local` no la carga y
ningún endpoint usa el writer.

## 4. Preparación de adaptadores SQLx

Separación explícita del código actual:

| Categoría | Ubicación | Reutilizable al aprobar |
| --- | --- | --- |
| Dominio y políticas | `pos-core::identity`, `pos-core::audit` | Sí, sin cambios |
| Verificación criptográfica | `pos-credentials`, `credential_lab` | La verificación sí; `credential_lab` se sustituye |
| Consultas de identidad | `identity_lab` (`identity_*_lab`) | La lógica sí; los nombres de tabla no |
| Pruebas | `*_tests` con esquemas `*_lab` | Solo pruebas |
| Writer de auditoría | `audit_writer` | Sí, cuando `0006` se apruebe |

Plan de sustitución (sin dos implementaciones productivas):

1. Promover `0002`–`0007` a `migrations/local/`.
2. Renombrar las consultas de `identity_lab` a tablas canónicas
   (`identity_session_lab → local_session`, `identity_user_lab → pos_user`,
   `identity_role_lab → role`, `identity_role_permission_lab → role_permission`,
   `identity_user_role_lab → user_organization_role`,
   `identity_membership_lab → branch_membership`,
   `identity_membership_role_lab → branch_membership_role`).
3. Mover el adaptador a `pos-storage-sqlite::identity` (sin `_lab`) y eliminar
   `ensure_lab_identity_schema`.
4. Repuntar `credential_lab` a `local_credential` y borrar su esquema de fixture.
5. Retirar `identity_lab` y `credential_lab` en el mismo cambio que activa el
   adaptador canónico.

## 5. Bootstrap y autorización

Secuencia conservada y probada en 1J/1M:

```text
organization → installation_identity → bootstrap_record(PENDING)
  → pos_user/role → bootstrap_record(COMPLETED) → audit_event(SYSTEM) → COMMIT
```

- Un fallo de auditoría revierte organización, instalación y bootstrap.
- Un segundo intento de completar la misma instalación no actualiza nada.
- Una instalación no puede completar con la organización de otra.
- El actor `SYSTEM` del bootstrap solo puede originarse en
  `AuditEventRecord::bootstrap_completed`.
- No hay endpoint de bootstrap ni secreto de producción.

## 6. Estado de D-AUTH / D-INST

| Decisión | Estado | Efecto en este incremento |
| --- | --- | --- |
| D-AUTH-01..08, 10 | Pendiente de firma | Bloquea la promoción |
| D-AUTH-09 | Bloqueada por benchmark | No afecta al SQL; impide Auth productivo |
| D-AUTH-11 | Pendiente; recomendación A | Sin cambio físico |
| D-INST-01 | Pendiente | `status` solo `ACTIVE` |
| D-INST-02 | Pendiente | Sin `UNIQUE` en la huella |
| ADR-0022 | Pendiente | Sin cambio |

El expediente `rust-1i-approval-decision-record.md` (1M) es el documento de
decisión; este incremento no lo modifica ni declara aprobación alguna.

## 7. Pruebas ejecutadas (Windows, SQLite real)

```bash
cd backend-rust
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo build --locked --workspace --all-features
```

| Resultado | Valor |
| --- | --- |
| `cargo fmt --check` | OK |
| `cargo clippy -D warnings` | OK |
| `cargo test` | **140 pruebas, 0 fallos** |
| `cargo build` | OK |
| Base vacía → `0001` + propuestas | OK |
| Reapertura y durabilidad | OK |
| Dos organizaciones / dos sucursales | Cubierto en `proposals_tests` |
| CAS obsoleto y rollback | Cubierto |
| Auditoría append-only y rollback | Cubierto |

No se ejecutó CI, Linux/WSL2 ni PostgreSQL 17. No se ejecutaron pruebas Java.
`--require-complete`, DDL13 y Flyway permanecen intactos.

## 8. Riesgos que impiden la promoción

1. Falta la firma del bloque 7 del expediente 1M.
2. D-INST-02 sin decidir: dos instalaciones podrían declarar la misma huella.
3. D-AUTH-11: la unicidad de email es semántica solo si todas las escrituras
   pasan por el adaptador; una escritura SQL directa puede crear duplicados.
4. D-AUTH-09: Argon2id comercial sin benchmark ni interoperabilidad validada.
5. El writer de auditoría depende de que el esquema candidato no cambie durante
   la aprobación.

## 9. Plan de activación tras la aprobación

1. Mover `0002`–`0007` a `backend-rust/migrations/local/` conservando orden.
2. Ejecutar `cargo test` con `sqlx::migrate!` real y una base vacía.
3. Sustituir `identity_lab`/`credential_lab` por el adaptador canónico (§4).
4. Activar `SqliteAuditWriter` en el composition root de `pos-local`.
5. Implementar el bootstrap one-shot con guard de secreto pre-body.
6. Componer el flujo administrativo completo con pruebas de extremo a extremo sin
   red.

## 10. Próximo flujo funcional

Con las migraciones aprobadas: **Bootstrap → Auth → RBAC → Organization/Branch →
Audit**, con un administrador real creando una sucursal y un usuario, resolviendo
permisos desde SQLite y registrando ambos hechos en `audit_event` dentro del mismo
commit.
