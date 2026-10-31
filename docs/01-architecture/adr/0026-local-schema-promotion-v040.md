# ADR-0026 — Promoción del esquema físico Local para v0.4.0

**ACEPTADA para la release técnica v0.4.0.** Registra la aceptación expresa de
las decisiones físicas que mantenían `0002`–`0017` como propuestas y su
promoción al migrador productivo.

No aprueba la política comercial de Argon2id (D-AUTH-09), no activa comandos IPC
comerciales y no convierte el esquema en inmune a cambios futuros: toda
corrección posterior exige una migración nueva.

## Contexto

El migrador productivo (`sqlx::migrate!("../../migrations/local")`) embebía solo
`0001_organization_branch.sql`. El resto del modelo —identidad, credenciales,
sesiones, RBAC, memberships, auditoría, bootstrap, catálogo, cajas, turnos,
ventas, inventario, recetas, checkout, compras y autorización de provisioning—
vivía en `backend-rust/proposals/` y solo se aplicaba desde pruebas y desde el
arnés sintético `pos-lab`.

Esa separación impedía cualquier producto funcional: `pos-local` y el escritorio
no podían crear un administrador, iniciar sesión ni operar caja, ventas o
inventario. Los adaptadores, casos de uso y pruebas ya existían y se ejecutaban
sobre SQLite real.

La misión de transición (2026-10-31) autoriza completar pruebas y migraciones y
promover el conjunto aprobado una vez satisfechas las pruebas. Esta ADR es el
registro de esa aceptación.

## Decisión

1. Se promueven `0002`–`0017` a `migrations/local/`, en orden de dependencia y
   sin cambios de contenido más allá de la cabecera de estado.
2. `0001_organization_branch.sql` no se modifica (byte a byte).
3. Las migraciones promovidas son **inmutables**. Cualquier cambio de columna,
   `CHECK`, índice o trigger posterior se expresa en una migración nueva.
4. `backend-rust/proposals/` deja de contener DDL; conserva únicamente un README
   que explica la promoción.
5. `backend-rust/proposals/README.md` y el tablero de migración son las
   referencias de estado.

## Resoluciones físicas aceptadas

| Bloque | Decisión aceptada | Implementación resultante |
| --- | --- | --- |
| D-INST-01 | Catálogo de `status` reducido a `ACTIVE` mientras no exista otro estado justificado | `CHECK (status='ACTIVE')` en `installation_identity` |
| D-INST-02 | No se impone unicidad global de `public_key_fingerprint`; la huella pública no acredita hardware | Sin `UNIQUE` adicional; longitud y ausencia de NUL validadas |
| D-AUTH-01/02 | Credencial local `PASSWORD`/`ARGON2ID`, versión 1, intentos fallidos y bloqueo explícitos | `local_credential` con `CHECK` de política y estado |
| D-AUTH-04/05 | Sesión persistida por digest, no por token en claro; estados y caducidad derivada | `local_session` + `CHECK` de coherencia y `closed_at` |
| D-AUTH-06 | Catálogo RBAC de 59 permisos idéntico a Java (V5–V25) y roles por organización o sucursal | `permission`, `role`, `role_permission`, `user_organization_role`, `branch_membership*` |
| D-AUTH-07/08 | Auditoría append-only con actor `USER`/`SYSTEM`, metadata acotada y sesión opcional | `audit_event` + triggers `no_update`/`no_delete` |
| D-AUTH-10 | Bootstrap one-shot por instalación con FK compuesta instalación↔organización | `bootstrap_record` |
| D-AUTH-11 | Opción A: normalización de email en la aplicación; SQLite solo exige valor recortado y forma básica | `CHECK` físico sin plegado Unicode |
| D-CAT-01/02 | Importes de catálogo como `TEXT` decimal escala 4; nombres y tablas singulares del contexto Local | `category`, `product` |
| D-CASH-01..04 | Caja por sucursal y turno con índice parcial `OPEN` único | `cash_register`, `cash_shift` |
| D-SALES-01..05 | Snapshots inmutables de línea, pagos inmutables y consumo de inventario en la misma UoW | `sale`, `sale_item`, `payment`, snapshots de componentes |
| D-INV-01/02 | Cantidades `TEXT` escala 6 y ledger append-only por trigger | `inventory_item`, `inventory_balance`, `inventory_ledger`, recetas |
| CC21-03 | `unit_cost TEXT NULL` con semántica «desconocido ≠ cero» y total derivado `NULL` | `purchase_order_line`, `purchase_receipt_line` |
| ADR-0021 | Representación física de fichas, costo efectivo y snapshots de compras | `0016_purchasing.sql` |

`0002`–`0017` conservan las claves foráneas, los `CHECK`, los índices parciales y
los triggers de las propuestas originales; no se relajó ninguna restricción para
hacer pasar las pruebas.

## Evidencia

Ejecutada sobre SQLite real con el migrador productivo:

| Prueba | Qué demuestra |
| --- | --- |
| `schema_tests::productive_migrator_publishes_the_approved_schema` | Instalación limpia aplica 17 migraciones, crea todas las tablas del alcance, `PRAGMA foreign_key_check` vacío y `integrity_check = ok` |
| `schema_tests::upgrade_from_the_previous_schema_applies_the_new_migrations` | Un historial con solo `0001` sube a `0017` con el migrador productivo y conserva integridad |
| `schema_tests::productive_schema_applies_from_empty_database_and_is_one_shot` | Reejecutar el migrador no reaplica ni falla |
| `schema_tests::rbac_catalog_resolution_preserves_java_semantics` | Catálogo de 59 permisos y resolución organizacional/sucursal |
| `schema_tests::bootstrap_is_one_shot_per_installation_and_complete` | Bootstrap one-shot y FK compuesta |
| `schema_tests::bootstrap_authorization_models_preauthorization_and_append_only_events` | Autorización de provisioning y ledger append-only |
| `schema_tests::audit_contract_enforces_actor_session_and_append_only` | Auditoría append-only y metadata acotada |
| `pos-lab::tests::migration_set_is_dependency_ordered_and_referential` | Cada migración crea objetos y deja la integridad referencial limpia, aplicada una a una sobre `0001` |
| `cash_store_tests`, `sales_store_tests`, `checkout_store_tests`, `inventory_store_tests`, `purchasing_store_tests` | Casos de uso reales sobre el esquema promovido: CAS, aislamiento por organización/sucursal, concurrencia y rollback |
| `installation::tests::preparation_migrates_then_reports_reopen` y `reopen_rejects_a_changed_checksum` | Reapertura, checksum y rechazo de historial alterado |
| `backup::tests::future_or_changed_migration_history_cannot_be_published_as_backup` | Un historial alterado o futuro no se publica como backup |

`fmt` y `clippy --all-targets -D warnings` limpios; la suite completa del
workspace pasa en Linux (Rust 1.99.1).

## Consecuencias

- `approved_schema_version()` devuelve `17`. Un binario anterior rechaza una base
  con versión superior y un binario nuevo migra una base con historial `0001`.
- Las pruebas y el arnés ya no manipulan DDL candidato: usan el migrador real.
- `pos-local` y el escritorio pueden provisionar, autenticar y operar sobre el
  esquema aprobado, siempre que la capa IPC y la UI se implementen.
- Flyway/Java permanece como referencia histórica; no se modifica.

## Límites explícitos

- No se aprueba D-AUTH-09 (parámetros Argon2id comerciales); siguen en
  laboratorio hasta medir el hardware objetivo.
- No se activa ningún comando IPC comercial ni se elimina la frontera de
  laboratorio del escritorio.
- No se migran datos de instalaciones Java: no se ha identificado todavía una
  instalación con datos que lo requiera.
