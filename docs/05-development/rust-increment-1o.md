# Incremento 1O — Activación SQLx, adaptadores canónicos y seguridad de auditoría

**Estado: PREPARACIÓN VERIFICADA. Nada es productivo ni está aprobado.**

Rama: `feat/backend/rust-auth-activation-prep-1o`, basada en 1N
(`bff66510`). PR Draft apilado sobre #72.

## 1. Correcciones de seguridad (P0)

### 1.1 Inmutabilidad de `AuditEventRecord`

**Antes**: todos los campos eran `pub`, por lo que un evento podía cambiar de
actor, organización, sucursal, sesión o acción después de haber sido atribuido.

**Ahora**:

| Propiedad | Mecanismo |
| --- | --- |
| Atribución inmutable | Campos privados; sin setters |
| Lectura | Getters de solo lectura para los doce campos |
| Coherencia | `validate()` rechaza SYSTEM con usuario o sesión y USER sin usuario |
| USER | `from_verified_principal` toma organización, sucursal, actor y sesión del principal |
| SYSTEM | `bootstrap_completed` exige `&BootstrapProvisioning` |
| Pruebas | `assemble_for_tests` solo existe bajo `cfg(test)` |

### 1.2 Capacidad de bootstrap

`bootstrap_completed` ya no es alcanzable con un simple UUID. La capacidad se
emite únicamente con:

```text
BootstrapSecretVerifier (composition root)
  → authorize_bootstrap(verifier, secreto_presentado)
  → BootstrapProvisioning (campo privado, sin Clone, sin serialización)
  → AuditEventRecord::bootstrap_completed(&capacidad, ...)
```

Ocultar campos no bastaba: cualquier módulo podía fabricar un evento SYSTEM. La
capacidad obliga a pasar por el guard del secreto y no se transporta en DTOs.

### 1.3 Frontera transaccional del writer

**Antes**: `append_on(&mut SqliteConnection)` era público y permitía insertar la
auditoría fuera de una unidad de trabajo.

**Ahora**: la única API pública es `append(&mut SqliteTransaction, &AuditEventRecord)`.
La conexión se obtiene de la transacción y el helper interno es privado. El
writer no abre conexiones, no confirma y no acepta el pool.

## 2. Adaptadores canónicos

`SqliteIdentityLab` pasa a operar sobre un `TableSet`:

| Conjunto | Constructor | Tablas |
| --- | --- | --- |
| `Lab` | `SqliteIdentityLab::new` | `identity_*_lab` |
| `Canonical` | `SqliteIdentityLab::canonical` | `pos_user`, `local_session`, `role`, `role_permission`, `user_organization_role`, `branch_membership`, `branch_membership_role` |

La lógica de permisos (override de organización, pierna de sucursal, recuperación,
clasificación `Missing`/`Conflict`) es **una sola**; solo cambian las sentencias.
No hay dos implementaciones de negocio.

Ajustes que exige el esquema canónico y que el laboratorio ocultaba:

| Hallazgo | Corrección |
| --- | --- |
| `local_session` guarda timestamps ISO; el dominio usa segundos Unix | La consulta canónica normaliza con `CAST(strftime('%s', ...) AS INTEGER) AS created_at/expires_at/revoked_at` |
| `state` debe ser `REVOKED` cuando hay `revoked_at` | La sentencia canónica actualiza `state` y `revoked_at` juntos |
| `revoked_at` es texto, no entero | Nuevo `revoke_session_cas_at(..., revoked_at_iso)` para el dialecto canónico |

## 3. Bootstrap completo

`bootstrap_candidate::provision_initial_installation` ejecuta, en la transacción
recibida: organización → instalación → `PENDING` → `pos_user` → `role` →
`user_organization_role` → `COMPLETED` → auditoría SYSTEM. Un `UPDATE` que no
afecte exactamente una fila se considera un segundo intento y devuelve error, de
modo que el bootstrap es one-shot.

La clave privada de la instalación nunca se persiste; solo la huella pública.

## 4. Flujo administrativo de integración

`candidate_flow_tests::administrative_flow_bootstraps_authorizes_audits_and_revokes`
recorre los catorce pasos sobre SQLite real y sin red:

1. Provisionar organización inicial.
2. Crear instalación.
3. Provisionar administrador.
4. Crear rol y asignación.
5. Conceder permisos del catálogo (`organization.read`, `branch.create`, `branch.read`).
6. Sesión sintética con hash de 64 hex.
7. Resolver principal con el adaptador canónico.
8. Verificar `branch.create` concedido y `sale.checkout` denegado (deny-by-default).
9. Crear sucursal con `SqliteTransaction::insert_branch`.
10. Auditar `branch.created` con el actor del principal.
11. Confirmar el commit.
12. Comprobar `bootstrap_record = COMPLETED`, una sucursal y dos eventos.
13. Revocar con CAS: `Updated` y luego `Conflict`.
14. Reintentar autenticación: rechazada.

Segunda prueba: el segundo bootstrap se rechaza y no deja organizaciones parciales.

## 5. Activación de migraciones

`productive_migrator_does_not_discover_candidate_proposals` demuestra que tras
`migrate()` solo existe una migración aplicada (`0001`) y que ninguna tabla
candidata (`pos_user`, `local_session`, `audit_event`, `bootstrap_record`, …)
existe en el esquema productivo. Las propuestas no tienen feature flag: no hay
forma de ejecutarlas accidentalmente.

Procedimiento de promoción (tras la firma):

1. Copiar `0002`–`0007` a `backend-rust/migrations/local/` sin renombrar.
2. Ejecutar `cargo test` con una base vacía y con una base que ya tenga `0001`.
3. Ejecutar `sqlx migrate run` sobre una base temporal y revisar checksums.
4. Sustituir `SqliteIdentityLab::new` por el constructor canónico en el
   composition root y eliminar `ensure_lab_identity_schema`.
5. Activar `SqliteAuditWriter` y `provision_initial_installation`.
6. Retirar `identity_lab`/`credential_lab` en el mismo cambio.

## 6. Reconciliación de cobertura

1N reporta 140 pruebas y 1M 219 porque **no comparten ancestro**: 1N nace de
1I-prep; 1J–1L añadieron catálogo, cajas, inventario, compras, ventas, supplier y
recetas. No hay pérdida de cobertura.

Archivos que ambas cadenas modifican: `pos-core/src/lib.rs`,
`pos-storage-sqlite/src/lib.rs` y `proposals_tests.rs` (declaraciones de módulo y
pruebas). La integración es textual y aditiva; no hay cambios semánticos en
conflicto. La verificación combinada se hará en la rama de activación, cuando el
esquema esté aprobado y ambas cadenas puedan convivir.

## 7. Pruebas ejecutadas (Windows, SQLite real)

| Control | Resultado |
| --- | --- |
| `cargo fmt --all -- --check` | OK |
| `cargo clippy -D warnings` | OK |
| `cargo test --locked --workspace --all-features` | **142 pruebas, 0 fallos** |
| `cargo build --locked --workspace --all-features` | OK |

Cubierto: inmutabilidad de atribución, capacidad de bootstrap, writer en UoW,
rollback, append-only tras reapertura, metadata canónica sin truncamiento,
timestamps con y sin fracción, bootstrap one-shot, RBAC DB-authoritative,
deny-by-default, CAS de sesión y reautenticación rechazada.

No ejecutado: CI, Linux/WSL2, PostgreSQL 17, pruebas Java. `--require-complete`,
DDL13 y Flyway intactos.

## 8. Estado normativo

D-AUTH-01..08, D-AUTH-10, D-AUTH-11, D-INST-01, D-INST-02 y ADR-0022 siguen
**pendientes**. D-AUTH-09 sigue bloqueada. Ninguna decisión se marcó aprobada.

## 9. Riesgos

1. La capacidad de bootstrap acredita que un verificador aceptó el secreto; la
   calidad del verificador depende del composition root.
2. El adaptador canónico depende de que `0003`–`0005` no cambien durante la
   aprobación.
3. `strftime('%s', ...)` asume el formato ISO exacto del `CHECK`; un cambio de
   formato exigiría revisar el adaptador.
4. Sigue sin existir verificación criptográfica productiva ni política Argon2id
   comercial.

## 10. Siguiente implementación funcional

Con la firma: PR de activación con las migraciones en `migrations/local/`, el
adaptador canónico como único, el writer y el bootstrap conectados al composition
root, y el flujo administrativo ejecutándose de extremo a extremo con un
administrador real.
