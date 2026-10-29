# migration-lab

Laboratorio local de migraciones con **datos exclusivamente sintéticos**. Los
checks técnicos SQLite y los casos normativos se reportan por separado. Pasar
los primeros **no** certifica paridad Java/Rust, PostgreSQL/SQLite, seguridad,
stock, dinero ni el esquema completo.

## Ejecución

Desde `backend-rust/`, con el workspace y `pos-storage-sqlite` disponibles:

```sh
cargo run -p migration-lab -- --output target/migration-evidence
cargo run -p migration-lab -- --output target/migration-evidence-complete --require-complete
cargo test -p migration-lab
```

El directorio de salida **no debe existir** y su padre debe existir. El runner
usa `create_dir` (no `create_dir_all`) y `create_new` para los artefactos; no
sobrescribe ejecuciones anteriores, no crea padres, no acepta `..` en la ruta y
no admite URLs de bases de datos. También acepta rutas absolutas explícitas.
Los fixtures SQLx se copian a directorios temporales propios cuando la prueba
necesita modificar una migración; nunca se modifican las fuentes.

Códigos de salida:

- `0`: los checks técnicos pasan; puede haber normativos bloqueados.
- `1`: fallo técnico, de argumentos, procedencia o escritura de evidencia.
- `2`: técnicos correctos, pero cobertura normativa incompleta con
  `--require-complete`.

## Qué se ejecuta realmente

- `open` de storage en archivos temporales, con timeout de SQLite explícito.
- Fixture `0001`: `lab_parent` y `lab_child`, ambas `STRICT`, con filas sintéticas.
- Adquisición simultánea de **todas** las conexiones del pool; `foreign_keys=1`
  e inserción huérfana rechazada por cada conexión física.
- Rechazo de texto no numérico en una columna `INTEGER` de una tabla `STRICT`.
- Fixture `0002`: columna con default e índice; comparación ordenada de datos
  e identidades antes/después del upgrade.
- SQLx rechaza con `VersionMismatch(1)` una copia alterada de `0001`, sin cambiar
  el historial ni el esquema.
- `0003` deliberadamente inválida: el rollback elimina la tabla recién creada y
  conserva datos e historial; el fallo debe mencionar `lab_missing`.
- Dos reruns con historial completo idéntico, incluidos checksum,
  `installed_on` y `execution_time`, además de datos y esquema.
- Backup `VACUUM INTO` parametrizado desde WAL con frames reales y una fila
  confirmada en WAL; copia del backup cerrado, restauración, comparación de
  datos/esquema/historial, `integrity_check`, `foreign_key_check`, cierre y reopen.
- API real `migrate`/rerun/`checkpoint` de storage en otra BD aislada. Su esquema
  local es **solo un subconjunto F1-10**, no el baseline completo.
- Huellas de migraciones antes/después para detectar cambios durante el run.

Los digests de datos técnicos se calculan sobre JSON canónico del laboratorio:
arrays con filas ordenadas por ID, columnas explícitas, enteros y strings.
No se publican como hashes de datos de negocio.

## Evidencias

- `baseline-manifest.json`: HEAD de Git, dirty observado y hash de la salida
  porcelain (no hash del contenido del worktree), `flyway_worktree` con los paths
  de migraciones modificadas o untracked en el checkout, hashes SHA-256 de los
  **25 archivos Flyway leídos**, y las **37 tablas obtenidas del SQL fuente**.
  El inventario excluye `flyway_schema_history`, ignora comentarios/literales/
  cuerpos dollar-quoted y rechaza `DROP TABLE` o `CREATE TABLE` no soportado.
  No es una consulta a una BD viva ni un checksum CRC32 de Flyway.
- `migration-manifest.json`: archivos/versiones/digests reales de los roots
  `local`, `central`, `control`; versiones resueltas mediante
  `cargo metadata --offline --locked`, hash de Cargo.lock, rustc/cargo y versión
  SQLite consultada. Una observación no disponible se marca `NOT_RUN`/`null`.
- `parity-report.json`: `technical_checks` independientes de `normative_cases`.
  Los 30 casos `DDL13-01..11`, `PAR-01..09`, `AS-01..10` permanecen
  `BLOCKED`/`NOT_RUN`, nunca `PASSED` por estos fixtures. Un fallo técnico fija
  el estado global a `FAILED`, manteniendo `normative_status: BLOCKED`.
- `parity-report.xml`: incluye los checks técnicos y los 30 normativos con su
  cobertura identificada; los normativos son `skipped`, no éxitos vacíos. Un
  técnico fallido aparece como `failure` también en este informe principal.
  `technical-checks.xml` permite consultar solo las pruebas técnicas ejecutadas.
- `counts.json`, `integrity.json`, `money-vectors.json`,
  `stock-reconciliation.json`, `idempotency.json`, `auth-transition.json`,
  `security-report.json`: estado normativo explícito y, cuando corresponde,
  observaciones técnicas identificadas como tales.
- `schema-snapshots/`: snapshots SQLite observados y sus hashes técnicos;
  snapshot normativo completo pendiente.
- `backup-fixtures/`: backup, restore, `restoration-report.json` y
  **`restore.sha256` técnico**, solo si la restauración fue verificada. Si falla,
  el archivo contiene JSON explicativo sin digest.
- `ids.sha256`, `snapshots.sha256`, `restore.sha256` en la raíz: **JSON BLOCKED
  sin digest**, porque no se ejecutaron exportaciones/restores normativos.
  No deben procesarse como archivos `sha256sum` hasta existir esa evidencia.
- `immutability-tests.xml`: historial/fuentes técnicos y un caso normativo de
  inmutabilidad `skipped`.
- `failure-logs/`: errores SQL esperados comprobados y fallos técnicos reales
  separados. `run-summary.md`: alcance, estados y código de salida.

## Limitaciones deliberadas

- `local/0001` (organization/branch), `central/0001` (operational_tenant) y
  `control/0001` (customer) son **subconjuntos**, no el esquema F1-10 completo.
- No hay importación de datos reales, comparación entre motores, vectores de
  negocio completos ni validación de contratos de auth o permisos.
- PostgreSQL 17.11 se ejecuta de forma nativa y aislada cuando se pasa
  `--postgres-bin`; no se usa Docker, `DATABASE_URL` ni un servidor externo. Sus
  19 checks técnicos no completan los esquemas Central/Control.
- El origen Java/Flyway se ejecuta con `--java-source`; exporta 37/37 tablas y
  prueba PAR-08 con dos instalaciones y digest idéntico. No importa al target.
- DDL13-10 está **`NOT_RUN`**: el runner no certifica el aislamiento
  `network-denied` ni valida el esquema completo. Un wrapper externo puede
  adjuntar una observación técnica sin aprobar ese gate. `cargo metadata
--offline` y no consultar PostgreSQL no acreditan por sí solos aislamiento.
- El workspace/dependencias, storage, core y CI son prerrequisitos externos a
  este paquete; este lab no los modifica ni los reemplaza por mocks.

### Comprobación manual externa de aislamiento

Un wrapper Python externo lanzado con `unshare --user --map-root-user --net`
puede ejecutar el binario ya compilado y adjuntar observaciones del namespace.
La ejecución manual comunicada observó únicamente la interfaz `lo`, rutas IPv4
vacías y 11 checks técnicos SQLite con salida `0`; su sidecar está en
`target/migration-offline-evidence/offline-execution.json` (ruta relativa a
`backend-rust/`). El sidecar es evidencia externa, no una certificación generada
ni validada por el runner, y este no lo consume para aprobar casos normativos.

Esta comprobación manual aislada no constituye un harness automatizado y
mantenido ni demuestra operación sostenida del runtime con el esquema completo.
No valida por sí sola el dominio, paridad entre motores o todos los escenarios
de red. Puede adjuntarse como observación técnica separada; **DDL13-10 conserva
`NOT_RUN` y ningún normativo pasa a verde**. No se sobrescriben los informes de
ejecuciones anteriores para incorporar esta observación.
