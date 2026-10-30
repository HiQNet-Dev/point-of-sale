# Incremento 1Q — CI real, desbloqueo monetario y primer flujo comercial persistente

**Estado: técnicamente cerrado. Ninguna decisión normativa fue aprobada por el agente.**

Este informe registra exclusivamente resultados ejecutados. Distingue, en cada
caso, la evidencia real de la inferencia.

## 1. Preflight verificado

| Referencia | Valor comprobado |
| --- | --- |
| Rama de partida | `feat/backend/rust-consolidation-1p` |
| HEAD 1P | `21bcfd7ec6d21245a567ae2daca0506b0376fffc` |
| PR de consolidación | #74 (Draft, base #73) |
| Worktree | Limpio antes de empezar |
| `dev` | Ancestro de 1P (`329c0984`); no se modificó |
| `main` | `c290f518`; no se modificó |

## 2. CI real sobre la consolidación (P0)

El workflow `.github/workflows/ci.yml` solo dispara en `push`/`pull_request`
hacia `main`/`dev`, y `workflow_dispatch` exige `refs/heads/main`. Por eso la pila
#67–#74 nunca ejecutó CI. Se creó una rama **de validación sin cambios de código**
desde el HEAD exacto de 1P y un PR Draft contra `dev`:

| Elemento | Valor |
| --- | --- |
| Rama | `chore/ci-validation-1q` (mismo árbol que 1P) |
| PR | #75, Draft, base `dev`, propósito único de validación |
| Ejecución | [run 40937623585](https://github.com/HiQNet-Dev/point-of-sale/actions/runs/40937623585) |
| Evento | `pull_request` |
| Runner | `ubuntu-latest` (primera validación Linux del código consolidado) |

### 2.1 Resultado por trabajo

| Trabajo | Resultado | Motivo |
| --- | --- | --- |
| Detect Changes | ✅ success | Detectó cambios solo en `backend-rust` y `docs` |
| Backend (Java) | ⏭️ skipped | Sin cambios en `backend/` |
| Frontend | ⏭️ skipped | Sin cambios en `frontend/` |
| E2E (Chromium + PostgreSQL) | ⏭️ skipped | Condición de evento no aplica |
| Docker compose | ⏭️ skipped | Sin cambios |
| Rust | ❌ failure | **Solo** por el gate normativo (ver 2.2) |
| CI Gate | ❌ failure | Consecuencia del trabajo Rust |

### 2.2 Pasos del trabajo Rust

| Paso | Resultado |
| --- | --- |
| Validate requested SHA input | ✅ |
| Verify checked-out target and committed lockfile | ✅ |
| Install verified secret scanner + Scan Rust sources | ✅ |
| Install Rust toolchain | ✅ |
| Test evidence verifier integrity checks | ✅ |
| `cargo fmt --all -- --check` | ✅ |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | ✅ |
| `cargo test --locked --workspace --all-features` | ✅ |
| `cargo build --locked --workspace --all-features` | ✅ |
| Build pinned PostgreSQL 17 | ✅ |
| Generate technical and native migration evidence | ✅ |
| Verify technical evidence | ✅ |
| `cargo audit` sobre `Cargo.lock` | ✅ |
| **Require complete normative migration evidence (`--require-complete`)** | ❌ **bloqueo normativo esperado** |
| Verify blocked gate evidence (`--expect-exit 2`) | ✅ |
| Upload evidence artifacts | ✅ |

**Conclusión.** En Linux, todo el código consolidado de 1P pasa formato, Clippy,
pruebas, build, escaneo de secretos, auditoría de dependencias y evidencia técnica
de migración. El único fallo es el gate DDL13/normativo, que sigue bloqueado por
las decisiones pendientes del titular. No se modificó `--require-complete`, no se
añadió `continue-on-error` ni `|| true`, y no se declaró CI verde.

## 3. Decisiones normativas (sin cambios de estado)

Siguen **PENDIENTES** y ahora incluyen el bloque del catálogo:

| Bloque | IDs | Estado |
| --- | --- | --- |
| Auth/RBAC/Audit | D-AUTH-01..08, D-AUTH-10, D-AUTH-11 | PENDIENTE |
| Instalación | D-INST-01, D-INST-02 | PENDIENTE |
| Reconciliación | ADR-0022 | PENDIENTE |
| Argon2id comercial | D-AUTH-09 | BLOQUEADA |
| Catálogo Local | D-CAT-01..03 | PENDIENTE (nuevo, independiente de Auth) |

El expediente `docs/05-development/rust-1i-approval-decision-record.md` incorpora
§2.2 (catálogo), la fila `0008` en §3 y las filas de respuesta en §7. El bloque 7
sigue vacío: **no se atribuyó ninguna aprobación al titular**.

## 4. Migraciones `0002`–`0007`

Sin cambios: siguen en `proposals/local-auth/` y `pos-local` no las descubre. No
se promovió ninguna migración porque no existe aceptación formal.

Nueva propuesta independiente:

| Archivo | Depende de | Estado |
| --- | --- | --- |
| `proposals/local-catalog/0008_catalog.proposed.sql` | `0001` | Propuesta, fuera del migrador |

## 5. Corrección de tipos monetarios (P1)

Se identificaron dos familias sin conversión autorizada: `MoneyMinor` (`i64`,
unidades menores) y `catalog::Decimal4` (`i128`, escala 4). Se añadió el único
puente admitido:

| Elemento | Archivo |
| --- | --- |
| `MinorExponent` (0..=4, sin tabla ISO implícita) | `pos-core/src/money_scale.rs` |
| `minor_to_decimal4` | idem |
| `decimal4_to_minor` | idem |
| `MoneyScaleError` (`UnsupportedExponent`, `OutOfRange`, `LossOfPrecision`, `Overflow`) | idem |

Reglas:

- No existe conversión implícita: el exponente de la moneda es una entrada
  obligatoria y no se deduce del código ISO.
- Un valor como `1.0050` con exponente `2` devuelve `LossOfPrecision`; no se
  redondea nunca.
- Magnitudes `NUMERIC(19,4)` que exceden `i64` se reportan como `Overflow` o
  `OutOfRange`, no se truncan.

9 pruebas nuevas: exponentes 0/2/3/4, cero, negativos, ida y vuelta, rechazo de
fracción no representable, rango fuera de escala 4 y desbordamiento de `i64`.

## 6. Primer caso de uso comercial persistente (P1)

El catálogo es el primer módulo con **autorización real, transacción y auditoría
sobre SQLite real**.

### 6.1 Esquema propuesto

`0008_catalog.proposed.sql` transcribe Flyway V9 con estas diferencias
deliberadas, registradas como D-CAT:

| Aspecto | Java/PostgreSQL | Local propuesto |
| --- | --- | --- |
| Nombres | `categories`, `products` | `category`, `product` |
| Precio | `NUMERIC(19,4)` | `TEXT` canónico `0.0000` con `CHECK` de escala 4 |
| Timestamps | `TIMESTAMPTZ` | `TEXT` ISO UTC |
| Tenant | FK compuesta same-org | Idéntica, vía `UNIQUE (organization_id, id)` |
| Barcode | Índice único parcial | Idéntico |

El `CHECK` de `unit_price` rechaza texto libre, signos, más de un punto, más de
cuatro decimales, ceros iniciales y magnitudes fuera de 15 dígitos enteros.

### 6.2 Adaptador y casos de uso

| Pieza | Ubicación |
| --- | --- |
| `SqliteTransaction::{insert,get,list}_{category,product}` + CAS | `pos-storage-sqlite/src/catalog_store.rs` |
| `SqliteCatalogApplication` | idem |
| `NewCategoryRequest`, `NewProductRequest`, `CategoryUpdateRequest`, `ProductUpdateRequest`, `CatalogPage` | idem |
| `Permission::{CategoryCreate,CategoryRead,CategoryUpdate,ProductCreate,ProductRead,ProductUpdate,PosCatalogRead}` + `as_code()` | `pos-core/src/organization_branch.rs` |
| `ApplicationError` de catálogo | idem |

Autorización: los casos de uso reciben un `VerifiedPrincipal` y resuelven el
permiso contra `PrincipalScope::new(org, None)` con `PermissionCode`. Un principal
autenticado con scope de sucursal **no** autoriza el catálogo organizacional.
No hay `allow-all`, ni adaptadores de laboratorio en esta ruta.

Auditoría: `SqliteAuditWriter::append` sobre la misma `SqliteTransaction`, con
acciones `category.created`, `category.updated`, `product.created` y
`product.updated`, y metadata canónica (`code`, `sku`, `changedFields`).

### 6.3 Pruebas ejecutadas (9, SQLite real)

| Prueba | Qué demuestra |
| --- | --- |
| `category_and_product_persist_with_audit_and_survive_reopen` | Normalización (slug, SKU, precio, barcode con ceros, descripción en blanco), lectura, listado, dos hechos USER con sesión del principal, durabilidad tras reapertura |
| `product_price_update_uses_cas_and_skips_noop_audit` | CAS `0 → 1`, conflicto con versión obsoleta, precio intacto, no-op sin auditoría, metadata `{"changedFields":["unitPrice"]}` |
| `category_update_deactivates_and_blocks_product_assignment` | CAS de categoría, `CategoryInactive` al asignar, metadata `{"changedFields":["status"]}` |
| `duplicate_codes_skus_and_barcodes_are_scoped_per_organization` | Unicidad por organización para código, SKU y barcode; el mismo SKU es válido en otra organización |
| `category_of_another_organization_is_invisible_and_blocked_by_the_schema` | Lectura cross-org = inexistente; `Forbidden` sin permiso; **la FK compuesta rechaza la escritura SQL directa** |
| `audit_failure_rolls_back_the_catalog_write` | Sin `audit_event` el caso de uso falla y **no queda ninguna categoría** |
| `authorization_is_deny_by_default_and_requires_organization_scope` | Principal con rol de lectura no crea categorías; principal con scope de sucursal es rechazado aunque su rol tenga el permiso |
| `page_bounds_are_enforced` | `limit = 0` y `limit > 200` rechazados |
| `concurrent_price_updates_commit_only_one_revision` | Dos pools sobre el mismo archivo: solo un escritor confirma la revisión 0 |

## 7. Estado de los adaptadores canónicos

| Adaptador | Estado |
| --- | --- |
| `SqliteIdentityLab::canonical` (tablas candidatas) | Usado por los casos de uso de catálogo para resolver el principal |
| `identity_lab` (tablas `*_lab`) | Sigue existiendo solo para pruebas de laboratorio |
| `SqliteAuditWriter` | Usado por el catálogo; única API sobre `&mut SqliteTransaction` |
| `SqliteCatalogApplication` | Nuevo, sobre esquema candidato |
| `pos-local` composition root | **Sin cambios**: no monta catálogo, auth ni auditoría |

No se eliminaron `identity_lab`/`credential_lab` porque todavía no existe DDL
aprobado. La sustitución sigue condicionada a D-AUTH-01..08.

## 8. Ejecutable administrativo

No se construyó. El tramo `Bootstrap → Auth → RBAC → Organization/Branch → Audit`
está compuesto y probado sobre el esquema candidato
(`candidate_flow_tests::administrative_flow_bootstraps_authorizes_audits_and_revokes`),
pero montarlo en `pos-local` sin DDL aprobado habría supuesto exponer seguridad
sobre tablas no aceptadas. Sigue bloqueado por D-AUTH y por la elección de
transporte Local (HTTP vs IPC), que no se decidió unilateralmente.

## 9. Caja y turnos

No implementado. El dominio (`pos-core::cash`) y sus pruebas existen desde 1J; la
persistencia requiere el índice único parcial de turno abierto por caja y la
consulta de ventas abiertas, que aún no tiene DDL. Se mantiene como siguiente
frente P1 tras el catálogo, sin bloquearlo.

## 10. Calidad ejecutada

| Control | Resultado |
| --- | --- |
| `cargo fmt --all -- --check` | ✅ |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | ✅ |
| `cargo test --locked --workspace --all-features` | ✅ **246 pruebas, 0 fallos** |
| `cargo build --locked --workspace --all-features` | ✅ |
| Linux (CI, ubuntu) fmt/clippy/test/build | ✅ (run 40937623585) |
| Secret scan y `cargo audit` | ✅ (CI) |
| PostgreSQL 17 / Migration Lab | ✅ técnico; gate normativo bloqueado |
| Windows SQLite real | ✅ 246 pruebas |

Distribución: `pos-core` 125 · `pos-storage-sqlite` 75 · `migration-lab` 27 ·
`pos-local` 11 · `pos-credentials` 8. Incremento: +18 (9 monetarias, 9 catálogo).

No se ejecutaron pruebas Java ni E2E en esta sesión: no hubo cambio en `backend/`
ni en `frontend/`, y la paridad Java/Rust del catálogo se contrastó por lectura de
`CategoryService`, `ProductService` y Flyway V9 (acciones, permisos, normalización
y campos modificados), no por ejecución.

## 11. Riesgos

### P0

Ninguno nuevo. Las dos vulnerabilidades de 1O (atribución mutable y writer fuera
de la UoW) siguen cerradas y las nuevas pruebas las ejercen de forma indirecta al
exigir `VerifiedPrincipal` + `SqliteTransaction`.

### P1

| Riesgo | Efecto | Mitigación |
| --- | --- | --- |
| Catálogo sobre esquema candidato | No es producto | D-CAT + promoción separada de `0008` |
| Dos familias monetarias | Riesgo de conversión ad hoc | Puente único en `money_scale`; prohibido convertir fuera de él |
| Caja sin DDL | Bloquea turnos y ventas | Propuesta e índice único pendientes |
| API/OpenAPI sin empezar | Sin contrato externo | Tras aprobación de DDL y transporte |

### P2

| Riesgo | Efecto |
| --- | --- |
| `SqliteTransaction::connection()` público | Escape hatch usado por audit writer y catálogo; convendría un acceso `pub(crate)` cuando se estabilice la composición |
| Lecturas con `BEGIN IMMEDIATE` | Serializa lecturas de catálogo con escrituras; aceptable con pool de una conexión, revisable en Pro |

## 12. Siguiente paso

1. Respuesta del titular en el bloque 7 del expediente (D-AUTH, D-INST, ADR-0022,
   D-CAT).
2. PR de activación contra `dev`: mover `0002`–`0008` a `migrations/local`,
   conectar `SqliteCatalogApplication` y `SqliteAuditWriter` en `pos-local` y
   eliminar las tablas `*_lab` en el mismo cambio.
3. Con el esquema aprobado: caja → turno → venta → checkout.
