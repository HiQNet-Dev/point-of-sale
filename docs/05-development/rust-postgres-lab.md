# Laboratorio PostgreSQL nativo de migraciones Rust

## Alcance y estado

`backend-rust/tools/migration-lab/src/postgres.rs` implementa un laboratorio
**técnico**, con PostgreSQL 17 real y SQLx, sin Docker, servicios del sistema,
URLs de conexión externas ni datos productivos. Se consultó
[F1–13](https://docs.google.com/document/d/1ZtDnxOeVjeBJD20gLI1GxA4cTY-bevRoL2fKUMqZBPk/edit)
mediante su exportación de texto.

Los roots existentes `migrations/central/0001_operational_tenant.sql` y
`migrations/control/0001_customer.sql` son **parciales**. Ejecutarlos correctamente
no aprueba DDL13-02/03. Los esquemas completos de 82/83 tablas no están
implementados/verificados aquí; la cobertura normativa permanece `BLOCKED`.
Este módulo no acredita PAR-01..09, AS-01..10, auth migration, cutover ni releases.

## Frontera de integración

La API interna es:

```rust
pub(crate) async fn run(
    bin: &std::path::Path,
    evidence: &crate::report::Evidence,
) -> Result<serde_json::Value, crate::LabError>
```

`bin` debe ser el directorio de **initdb, postgres, pg_ctl, pg_dump y pg_restore**.
Se comprueba la versión mayor 17 de los cinco ejecutables y la versión efectiva
`server_version_num`. La versión patch observada se incluye en el JSON, no se
inventa a partir de la ruta. Los tests pueden usar PostgreSQL 17.11 compilado
localmente; la API permite cualquier patch 17.

La integración ya está hecha: `main.rs` registra `mod postgres;`, invoca la API
solo con `--postgres-bin` y agrega los 19 checks al resultado técnico global. Si
se solicita PostgreSQL y falta el binario, el CLI falla; no hay omisión silenciosa.
`Cargo.toml` habilita la feature `postgres` de SQLx y `tokio/time`; no se declara
una feature de test que pudiera activarse con `--all-features` sin PostgreSQL.
La integración real se ejecuta por el CLI, no por un test que pudiera quedar
omitido.

El workspace ya declara SQLx `0.9.0` con `runtime-tokio`, `sqlite`, `migrate` y
`macros`, Tokio `1.54.1`, serde_json `1.0.154`, tempfile `3.27.0`, sha2 `0.11.1`
y serde `1.0.229`. El módulo reutiliza esas dependencias y
`crate::provenance::sha256`; no requiere `libc`, TLS, shell, Docker ni otra crate.
Necesita Unix (`std::os::unix`), no es un backend Windows.

### Resultado y fallos

El valor retornado contiene:

- `technical_status`: `PASSED` o `FAILED`.
- `technical_checks`: array de `{id, status, coverage, detail}`; `detail` es JSON
  estructurado, **no un string**. El runner lo serializa al adaptarlo a
  `report::Case`. Todos los checks tienen cobertura `synthetic_technical`.
- `runtime`: versiones de binarios y servidor, transporte, HBA y permisos.
- `contexts.central/control`: snapshots clean/N-1/N, histories, privilegios y
  restore; los fallos de contexto no se representan como aprobaciones.
- `cleanup`: pools cerrados, hijo recolectado y namespace temporal eliminado.
- `normative_status: "BLOCKED"`, `normative_complete: false` y blockers explícitos.

Un binario inexistente, PostgreSQL no 17 o un fallo de arranque devuelve `Err` y
escribe `failure-logs/postgres-startup.json`. **Si se solicita PostgreSQL y no hay
binarios, el CLI falla, nunca lo omite ni reporta PASSED.** Los fallos durante
los checks se devuelven como checks `FAILED`; el runner los propaga a su exit
code. No basta con comprobar que `run()` devolvió `Ok`. Errores de escritura de
evidencia también devuelven `Err`.

Las categorías de error y SQLSTATE son seguros; no se serializan mensajes del
driver, DSNs ni variables de entorno. Los IDs de checks permiten localizar el
paso. El log del servidor solo corresponde al clúster sintético propio.

## Aislamiento, identidades sintéticas y limpieza

- Cada ejecución crea `/tmp/hiqnet-pg-*` con modo `0700`, y un subdirectorio de
  socket `0700`. La ruta corta evita el límite de `sockaddr_un` y no depende de
  un `TMPDIR` externo. `initdb --no-locale --encoding=UTF8` crea datos nuevos.
- PostgreSQL usa `listen_addresses=''`, solo socket UNIX privado y permisos de
  socket `0700`. El número 5432 identifica el archivo de socket **dentro de ese
  namespace**, no un listener TCP ni una reserva de puerto de red.
- `pg_hba.conf` solo contiene `local all all trust`. Trust está limitado al
  namespace efímero accesible por el usuario OS propietario; no es una receta
  para producción. Otro proceso del mismo UID comparte esa frontera OS.
- Los procesos hijos reciben un entorno vacío, con solo locale, zona horaria y
  HOME del laboratorio. SQLx no consulta `.pgpass`, tiene host/socket, puerto,
  usuario, database, password vacío y TLS deshabilitado explícitos. No se acepta
  ni consulta `DATABASE_URL`. `PGOPTIONS` se rechaza porque SQLx 0.9 lo hereda;
  nunca se imprime su contenido.
- `lab_admin` es un bootstrap **superusuario sintético**, sin contraseña, usado
  para crear roles/bases, verificar el aislamiento y ejecutar dump/restore.
  **No ejecuta las migraciones evaluadas**.
- `central_migrator`/`control_migrator` son propietarios de sus bases, sin
  SUPERUSER, BYPASSRLS, CREATEDB, CREATEROLE ni REPLICATION. Ejecutan SQLx.
- `central_app`/`control_app` son LOGIN NOINHERIT, sin esos privilegios, sin
  membresías administrativas ni ownership. Solo reciben USAGE en `lab` y CRUD
  sobre su fixture RLS. El GUC `lab.tenant` es sintético y controlado por el test:
  no autentica al tenant ni constituye una frontera de autorización productiva
  (la app puede cambiarlo). No acredita AS-03. No hay contraseñas, hashes ni
  credenciales reales.
- Se revoca CONNECT público. App y migrador no pueden conectar al otro contexto
  ni a source. Las bases `hiqnet_lab_central`, `hiqnet_lab_control` y
  `hiqnet_lab_source` son distintas; también hay bases separadas `*_restore`.
- `source` se crea vacía y se verifica que siga vacía. No se ejecuta Flyway ni se
  duplica el exportador. El JSON solo devuelve su nombre y alcance, **no opciones
  de conexión**: el clúster ya está destruido al retornar. Source/Flyway necesita
  su propia ejecución en el runner principal.

Los pools tienen máximo una conexión cada uno y quedan registrados **antes del
primer await**. Las conexiones administrativas temporales se cierran enseguida.
Los pools se cierran explícitamente antes de parar el servidor en éxito o error.
`Drop` es el respaldo para retorno temprano, cancelación o unwind: drena los
pools en un hilo/runtime acotado, para PostgreSQL con `pg_ctl -m immediate`, y el
guard del hijo siempre hace kill/wait. Después se elimina el directorio temporal.
Como cualquier RAII, esto no garantiza limpieza si se mata al propio runner con
SIGKILL o se pierde alimentación.

SQLx 0.9 devuelve algunos errores antes de liberar el advisory lock del migrador.
Las pruebas de checksum y DDL fallido usan una conexión dedicada con
`close_on_drop()` y cierre explícito, evitando devolver una sesión bloqueada al
pool. No se deshabilitan los locks ni se modifica el historial para hacer pasar
los tests.

Límites: versión de ejecutable 5 s, initdb 45 s, readiness 15 s, adquisición de
conexión 3 s, statement 10 s, lock 3 s, dump/restore 30 s y ejecución async 180 s.
El timeout async no interrumpe una llamada de proceso síncrona: esas llamadas
están acotadas por sus propios límites. `pg_ctl` tiene 5 s internos/8 s externos;
en Drop el drenaje tiene 3 s por pool. Estos límites y el arranque/cierre pueden
sumarse; 180 s no es un límite absoluto de pared para toda la API.

## Checks ejecutados

Una ejecución completamente verde produce **19 checks**, no 19 casos normativos:

| Check                                                           | Verificación real                                                                                                                                               |
| --------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `postgres_native_17_private_socket`                             | Binarios/servidor 17, `inet_server_addr()` nulo, sin listen TCP, HBA local trust y permisos privados.                                                           |
| `postgres_separate_database_setup`                              | Bases y roles propios, source vacía.                                                                                                                            |
| `postgres_{central,control}_clean_partial_root` (2)             | BD sin tablas, root real por SQLx, tablas esperadas exactas e historial propio; rerun sin mutación.                                                             |
| `postgres_{central,control}_fixture_upgrade_preserves_data` (2) | N-1→N en fixtures, UUID/tenant/NUMERIC/payload preservados; columna e índice nuevos utilizables.                                                                |
| `postgres_{central,control}_checksum_version_mismatch` (2)      | Copia temporal alterada; exactamente `MigrateError::VersionMismatch(1)` y snapshot inalterado.                                                                  |
| `postgres_{central,control}_rerun_exact_history` (2)            | Dos reruns, historial exacto con checksum/installed_on/execution_time, schema y datos idénticos.                                                                |
| `postgres_{central,control}_migration_vs_app_privileges` (2)    | Flags no elevados; app rechazada al crear/alterar DDL, escribir ambos histories o asumir admin; RLS SELECT/UPDATE/INSERT cross-tenant y write propio permitido. |
| `postgres_{central,control}_ddl_rollback` (2)                   | DDL + UPDATE previo al error, exactamente migración 3/SQLSTATE 42P01; tabla parcial ausente, datos e historial idénticos, cero success de versión 3.            |
| `postgres_{central,control}_dump_restore_exact_sql_digests` (2) | `pg_dump` antes del fallo, `pg_restore` en otra BD, snapshots/digests SQL idénticos, dos filas y reapertura real.                                               |
| `postgres_independent_root_tracking`                            | Ambos roots desde 0001, checksums distintos e histories independientes.                                                                                         |
| `postgres_database_boundaries`                                  | Ocho rechazos reales CONNECT/42501 para roles migrador/app contra otro contexto/source; source sin tablas SQLx.                                                 |
| `postgres_cleanup`                                              | Pools cerrados, hijo recolectado y namespace eliminado antes de retornar.                                                                                       |

Los fixtures viven en `fixtures/postgres/`. Su tracking está en
`lab._sqlx_migrations` para no colisionar con 0001 del root parcial en
`public._sqlx_migrations`. Se verifica que los fixtures no cambien ese historial
público. Estos fixtures **no son** entidades del dominio ni un tercer root real.

Los snapshots de catálogo incluyen columnas/tipos/defaults, constraints, índices,
owner, ACL y RLS/policies. Los datos se obtienen mediante SQL `jsonb_agg` con
orden estable, guardando el **texto canónico del servidor** sin parsear números a
`f64`. Los digests son SHA-256 del contenido observado, incluyendo ambos
historiales. La proyección N-1 compara NUMERIC como texto exacto, no floats.
No se exige igualdad de bytes del dump, que puede incluir metadatos variables,
ni igualdad de timestamps de migración entre ejecuciones independientes.

## Evidencia

Se usa el directorio `Evidence` nuevo que ya proporciona el runner; sus escrituras
son `create_new`, sin overwrite. Nombres exclusivos del módulo:

- `postgres-technical-report.json`.
- `schema-snapshots/postgres-{central,control}-{clean,v1,v2}.json`.
- `failure-logs/postgres-{central,control}-expected-{checksum,ddl-failure}.json`.
- `failure-logs/postgres-process.log`, y `postgres-startup.json` si falla arranque.
- `backup-fixtures/postgres-{central,control}.dump` y
  `postgres-{central,control}-restore.json`.

Los dumps contienen únicamente datos sintéticos del laboratorio y los histories
observados. El módulo no escribe reportes normativos compartidos ni sus `.sha256`.
No deben usarse estos dumps como backups de negocio.

## Compilación local y pruebas

No se necesita acceso al socket Docker ni permisos root. El helper opcional
compila PostgreSQL **17.11** en el checkout:

```sh
# Desde backend-rust; requiere compilador C, make, bison/flex, curl, tar/bzip2,
# sha256sum, timeout y flock. Descarga ~21 MiB del sitio oficial, sin subir datos.
sh tools/migration-lab/scripts/build_postgres.sh
```

Destino: `backend-rust/target/pg17-build/install/bin`. El tarball oficial se
verifica con SHA-256
`dd27f2b3c59e73ed14aa3324901242bf69a032a6347805f274e6260322d42979`, consultado en
[postgresql-17.11.tar.bz2.sha256](https://ftp.postgresql.org/pub/source/v17.11/postgresql-17.11.tar.bz2.sha256).
No instala servicios, no usa sudo y no compila sobre un directorio de datos.
Logs de configure/make/install quedan en `target/pg17-build/`. Cada proceso del
build tiene timeout (120/600/120 s); curl tiene máximo 120 s. Un lock evita builds
concurrentes vía este helper. `POS_LAB_PG_BUILD_JOBS` controla paralelismo
(predeterminado 2). No se altera el Rust workspace.

Comandos reproducibles desde `backend-rust/`:

```sh
sh tools/migration-lab/scripts/build_postgres.sh
cargo test -p migration-lab postgres::tests
cargo run --locked -p migration-lab -- \
  --output target/pg-evidence \
  --postgres-bin target/pg17-build/install/bin --java-source
python3 tools/migration-lab/scripts/verify_evidence.py \
  --evidence target/pg-evidence --expect-exit 0
```

Los seis unitarios no requieren PostgreSQL. La ejecución real de los 19 checks se
hace por el CLI; no existe un `#[ignore]`, un early-return que finja PASS ni una
feature de test que se active sin binarios. Hay además una prueba de `run()` con
binarios ausentes que exige `Err` y evidencia de arranque `FAILED`. El runner
principal agrega los checks y propaga cualquier `FAILED` al exit code.
