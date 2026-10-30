# Incremento 1P — Consolidación de las cadenas Rust

**Estado: INTEGRACIÓN TÉCNICA VERIFICADA. Sin aprobación normativa ni promoción de DDL.**

Rama: `feat/backend/rust-consolidation-1p`, creada desde 1O (`d3d4ec3f`) y fusionada
con 1M (`0ce5bac9`). PR Draft de integración.

## 1. Punto de partida

| Cadena | HEAD | Contenido |
| --- | --- | --- |
| Comercial | `docs/rust-1i-approval-1m` `0ce5bac9` | Catálogo, caja, turnos, inventario, compras, ventas, supplier, recetas y expediente |
| Auth/Audit | `feat/backend/rust-auth-activation-prep-1o` `d3d4ec3f` | Writer de auditoría, activación SQLx, adaptadores canónicos, bootstrap y flujo administrativo |
| Ancestro común | `f41ad54e` | 1I-prep |

## 2. Integración

`git merge --no-ff docs/rust-1i-approval-1m` sobre 1O. Sin rebase ni force-push; el
historial de ambas ramas se conserva.

### 2.1 Conflictos encontrados

Un único conflicto real: `backend-rust/crates/pos-storage-sqlite/src/proposals_tests.rs`.
Ambas cadenas añadían bloques de pruebas al final del archivo.

Resolución: se conservaron **las dos** aportaciones, sin `ours` ni `theirs`:

| Origen | Pruebas conservadas |
| --- | --- |
| 1N | `audit_occurred_at_accepts_optional_fraction_and_rejects_junk` |
| 1O | `productive_migrator_does_not_discover_candidate_proposals` |
| 1M | `first_run_requires_organization_before_installation_and_commits_once`, `first_run_rolls_back_everything_when_audit_fails` |

Al eliminar los marcadores se perdió el cierre de la función de 1O; se restauró a
mano (`Ok(())` y `}`) y la suite lo verificó.

`pos-core/src/lib.rs`, `pos-storage-sqlite/src/lib.rs` y `README.md` fusionaron
automáticamente.

## 3. Módulos conservados

`pos-core`: `audit`, `cash`, `catalog`, `identity`, `inventory`,
`organization_branch`, `purchasing`, `recipe`, `sales`, `supplier`.

`pos-storage-sqlite`: `audit_writer`, `bootstrap_candidate`, `credential_lab`,
`identity_lab`, `proposals_tests`.

Organización/sucursal, identidad, RBAC, verificación de credenciales, writer de
auditoría, bootstrap candidato, catálogo, cajas, turnos, inventario, compras,
ventas, pagos, supplier, recetas y efectos de modificador están presentes y
probados.

## 4. Suite combinada

Ejecutada realmente sobre el árbol integrado (Windows, SQLite real):

| Crate / binario | Pruebas |
| --- | --- |
| `pos-core` | 116 |
| `pos-storage-sqlite` | 66 |
| `pos-credentials` | 8 |
| `pos-local` (lib, bin, integración) | 11 |
| `migration-lab` | 27 |
| **Total** | **228** |

`cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets
--all-features -- -D warnings` y `cargo build --locked --workspace --all-features`
en verde.

Cobertura relevante ya existente y ahora ejecutada conjuntamente: primer arranque
y rollback, bootstrap one-shot, RBAC contextual con override de organización,
recuperación, auditoría append-only, CAS de sesión, expiración y revocación,
catálogo y precios, pagos y cambio, cantidades y stock, supplier, recetas y
snapshots, compras y recepciones, aislamiento entre organizaciones y sucursales.

## 5. Revisión de arquitectura

| Comprobación | Resultado |
| --- | --- |
| Tipos monetarios | `MoneyMinor` (i64, genérico) y `catalog::{Decimal4, UnitPrice}` (i128, escala 4). Sin conversión implícita entre ambos (**P1**) |
| Representaciones de Supplier | Una sola (`pos-core::supplier`); `purchasing` solo usa su snapshot |
| Validación de permisos | Una sola lógica, dos conjuntos de sentencias (`TableSet`) |
| Implementaciones de identidad | Una (`identity_lab` con `TableSet`); `canonical` sustituye a `lab` al activar |
| Atribución de auditoría | Campos privados + capacidad `BootstrapProvisioning` |
| Secretos en `Debug` | `SecretHash`, `SessionHash` y `CredentialAttempt` redactados; el writer no registra valores |
| Scope de autorización | `VerifiedPrincipal::authorize(permiso, scope)`; scope no reutilizable |
| SQLite fuera de UoW | Writer y bootstrap solo por `SqliteTransaction`; el store de Organization sigue usando el pool para lecturas/seed (**P1**) |
| Dependencias entre crates | `pos-core` sin dependencias; almacenamiento depende del dominio; sin ciclos |
| Dependencias de prueba en producción | Solo bajo `#[cfg(test)]` |

### 5.1 P0

Ninguno detectado en la integración. Las dos vulnerabilidades de 1O siguen
cerradas y con prueba.

### 5.2 P1

1. Dos familias monetarias sin conversión explícita; conviene un adaptador único
   antes de persistir precios.
2. `SqliteOrganizationBranchStore` mantiene métodos sobre el pool; el composition
   root debe usar siempre las variantes transaccionales.
3. `strftime('%s', …)` del adaptador canónico asume el formato ISO del `CHECK`.

## 6. Adaptadores canónicos

Sin cambios respecto de 1O: `TableSet::{Lab, Canonical}`, timestamps ISO
normalizados en SQL, `state='REVOKED'` junto a `revoked_at`, revocación CAS con
instante ISO y writer restringido a la transacción. Nada se conecta al
composition root productivo.

## 7. Flujo administrativo integrado

La prueba `administrative_flow_bootstraps_authorizes_audits_and_revokes` sigue en
verde dentro del árbol combinado: bootstrap → administrador → sesión → principal →
RBAC → crear sucursal → auditoría → commit → revocación CAS → reautenticación
rechazada. Sin red y sin `allow-all`.

## 8. CI

**Causa raíz de que los PR apilados no ejecuten workflows**: `.github/workflows/ci.yml`
solo dispara en `push`/`pull_request` hacia `main` y `dev`, más
`workflow_dispatch`. Los PR #64–#73 apuntan a ramas feature, por lo que no
cumplen el trigger.

Hallazgo adicional: el job Rust (`rust_run`, `backend-rust`, Migration Lab con
`--require-complete`) existe en `dev`, pero la copia de `main` no lo incluye. Un
dispatch desde `main` no validaría Rust.

Acción realizada: `workflow_dispatch` desde `dev` con
`target_sha=723c1cbb1011fcecb7251eddf896885744661129`. Resultado: run `40936337891`
falló en el paso **Validate requested SHA input** de todos los jobs.

Causa exacta, leída del workflow: el dispatch manual solo se acepta desde `main`
(`if event == workflow_dispatch && ref != refs/heads/main → exit 1`). Como la
copia de `main` no contiene el job Rust, **no existe hoy un camino permitido para
ejecutar el CI Rust de una rama feature**. El gate normativo además falló al no
encontrar `backend-rust` porque el checkout quedó en `dev`.

Ningún control se debilitó: no se tocaron triggers, gates ni `--require-complete`.
Propuestas, en orden de preferencia y sujetas a aprobación del titular:

1. Abrir el PR de activación contra `dev` cuando exista firma: el trigger
   `pull_request → dev` usa el workflow de `dev`, que sí incluye el job Rust.
2. Permitir `workflow_dispatch` desde `dev` en `ci.yml` (cambio de workflow, sin
   tocar gates) para validar SHAs de ramas feature antes de abrir el PR.
3. Añadir `feat/backend/**` a `pull_request.branches` para que los PR apilados
   ejecuten el workflow de su rama base.

## 9. Decisiones normativas

Siguen pendientes sin cambios: D-AUTH-01..08, D-AUTH-10, D-AUTH-11, D-INST-01,
D-INST-02 y ADR-0022. D-AUTH-09 sigue bloqueada. El expediente 1M es el documento
de decisión y no se modificó.

Propuesta consolidada para el titular: aprobar en bloque D-AUTH-01..08, D-AUTH-10,
D-INST-01 y ADR-0022; aprobar D-AUTH-11 opción A con garantía física parcial
declarada; diferir D-INST-02 (unicidad de huella) hasta definir el rehost;
mantener D-AUTH-09 bloqueada.

## 10. Plan de reducción de PR apilados

| Paso | Acción | Requisito |
| --- | --- | --- |
| 1 | Integrar #68–#71 en una rama comercial única | Este PR ya la contiene |
| 2 | Integrar #72–#73 en la misma rama | Hecho en este PR |
| 3 | Cerrar #69, #70, #71 como superseded al aprobarse la consolidación | Revisión del titular |
| 4 | #67 → #68 pasan a ser ancestros de la rama consolidada | Sin merges a `dev` |
| 5 | PR de activación contra `dev` cuando exista firma normativa | Gates verdes |

## 11. Próximo PR de activación

1. Copiar `0002`–`0007` a `migrations/local/`.
2. Conectar el adaptador canónico como único y retirar `*_lab`.
3. Activar writer y bootstrap en el composition root.
4. Ejecutar el flujo administrativo de extremo a extremo.
5. Abrir el PR de activación contra `dev` con CI real.
