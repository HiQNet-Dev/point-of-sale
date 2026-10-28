# Source lab Java/Flyway sobre PostgreSQL 17

## Alcance y límites

Laboratorio **source-only**, sintético y autónomo, sin Docker ni servidores de
producción. Crea dos instalaciones PostgreSQL independientes, una base `source`
en cada una, ejecuta **Flyway API real V1–V25** y el mismo seed SQL, y compara sus
bundles funcionales. No arranca Spring ni genera el esquema desde entidades.
No traduce las migraciones a otro migrador ni inventa DDL de destino.

Es una **observación técnica**, no aprobación normativa de paridad. No conecta,
crea ni importa target mientras siga el conflict freeze. No modifica
`main.rs`, `postgres.rs`, Cargo, CI, gates ni reportes principales. Los módulos
PostgreSQL del runner principal siguen usando su propio UNIX socket, sin cambios.

Archivos de implementación:

- `backend-rust/tools/migration-lab/scripts/java_source_lab.py`: Python stdlib,
  CLI, procesos, permisos, dos instalaciones, comparación y evidencias.
- `backend-rust/tools/migration-lab/fixtures/source/SourceFixture.java`: runner
  source-file Java, Flyway, JDBC, exportador y aserciones de precisión viva.
- `backend-rust/tools/migration-lab/fixtures/source/seed.sql`: solo datos source
  sintéticos; **sin DDL**. Se ejecuta después de V25, en una transacción.
- `fixtures/source/test_java_source_lab.py` y `native_cleanup_test.py` bajo el
  mismo migration-lab: regresiones stdlib y cleanup con PostgreSQL real.

## Ejecución reproducible

Desde la raíz del repositorio, con Python 3, Java 21 y PostgreSQL **17.x** nativos
(`postgres`, `initdb`, `pg_ctl`, `pg_isready`). Ejecutar como usuario no-root.
En este entorno se verificó PostgreSQL **17.11**, ya disponible en la ruta de abajo.
El padre de `--output` debe existir y el directorio final **no debe existir**:

```sh
python3 -B backend-rust/tools/migration-lab/scripts/java_source_lab.py \
  --pg-bin backend-rust/target/pg17-build/install/bin \
  --output backend-rust/target/java-source-lab-evidence
```

Para repetir, elegir otro nombre nuevo; nunca se sobrescriben directorios ni
artefactos existentes. Solo se aceptan esas dos opciones: no hay URL, host,
usuario, contraseña ni base externa configurables.

El wrapper Maven resuelve el classpath runtime **desde `backend/pom.xml`**:

```text
backend/mvnw -B -q dependency:build-classpath -DincludeScope=runtime -Dmdep.outputFile=<temporal>/classpath.txt
java -cp <classpath-resuelto> backend-rust/tools/migration-lab/fixtures/source/SourceFixture.java
```

El orquestador pasa `LAB_PORT`, `LAB_PASSWORD`, `LAB_MIGRATIONS`, `LAB_SEED` y
`LAB_WORK` por el entorno, no por argumentos. No ejecutar el Java manualmente
contra una base ajena. Maven puede descargar el wrapper/plugins/dependencias si
no están cacheados (acceso de red a repositorios Maven); no descarga datos de
negocio ni necesita credenciales de aplicación. Classpath, copias byte-exactas
de migraciones y clústeres están en un `TemporaryDirectory` 0700 bajo
`backend/target`, no en archivos versionados. No se cambia el POM.

Códigos de salida:

- **0:** observaciones técnicas source completas, ambas reconstrucciones y
  repeticiones iguales; **no significa paridad normativa verde**.
- **1:** fallo técnico, timeout, interrupción, versión inválida o CLI/output no
  utilizable. Si la ejecución llegó al laboratorio, conserva un
  `source-report.json` de fallo; un fallo previo de validación puede no generarlo.
- **2:** ejecución source incompleta/bloqueada, por ejemplo tablas sin datos.
  Nunca se interpreta una tabla vacía como cobertura positiva.

## Aislamiento, secretos y cleanup

- Cada instalación usa su propio clúster temporal 0700, `listen_addresses`
  limitado a **127.0.0.1**, puerto libre seleccionado temporalmente y sin UNIX
  socket. JDBC necesita TCP; este listener no pertenece al servicio de producto.
- Existe una pequeña ventana entre seleccionar y abrir el puerto. Una colisión
  falla la instalación y ejecuta cleanup; no se adopta un servidor existente.
- `initdb` exige SCRAM tanto local como host. La contraseña administrativa es
  aleatoria por instalación: entorno privado para los hijos y stdin de
  `initdb --pwfile=/dev/stdin`; nunca argumentos ni archivo de contraseña.
  El runner confirma en vivo PostgreSQL 17 y el verificador SCRAM del rol propio.
- El entorno de hijos es una allowlist: no hereda `PGHOST`, URLs de aplicación,
  `PGPASSWORD`, variables JVM de inyección ni `LAB_*` externos.
- PostgreSQL no escribe logs persistentes; su salida se descarta. Maven/Java se
  capturan y no se publican. Los errores muestran solo etapa/clases/frames
  filtrados, nunca mensajes JDBC/Flyway, SQL, contraseñas ni hashes de usuario.
- **Cada subprocess finito tiene máximo 200 s** (algunos checks y stop tienen
  límites menores). PostgreSQL es el único hijo persistente mientras dura una
  instalación. Cada hijo tiene process group propio. Ante fallo/timeout se mata
  y espera el grupo del comando, incluidos descendientes Maven/Java.
- El `finally` de cada instalación intenta `pg_ctl stop -m fast`, luego
  kill/wait del hijo propio como respaldo. `SIGINT`/`SIGTERM` entran en el mismo
  cleanup. Nunca se busca ni se detiene PostgreSQL por nombre, puerto global o
  PID ajeno. Al salir se elimina el temporal. `SIGKILL` del orquestador o caída
  del host no permiten ejecutar `finally`: no se promete cleanup ante ellos.
- Output 0700 y **todos los JSON 0600**, creados de forma exclusiva. El bundle
  contiene hashes BCrypt sintéticos fijos de usuarios: tratarlo como artefacto
  privado, no subirlo a repos ni publicarlo en CI. No contiene contraseñas en
  texto plano, JWT ni refresh tokens. Ni manifest ni report incluyen valores de
  hashes de credenciales; los SHA-256 publicados son huellas de archivos/bundles,
  no verificadores de contraseñas. No usar el seed en instalaciones reales.

## Bundle canónico versión 1

`bundle-1.json` y `bundle-2.json` contienen:

- `version: 1`, `kind: synthetic-flyway-source`, `tables`: las **37 tablas source**
  exactas, ordenadas por nombre. `flyway_schema_history` no se cuenta como tabla
  de aplicación ni entra en el digest funcional.
- Cada tabla contiene `name`, `action`, `source_count`, `primary_key`, `columns`
  y `rows`. Las columnas siguen el orden físico source de `SELECT *`, incluidas
  columnas añadidas por `ALTER TABLE`; cada descriptor tiene `name`, tipo
  PostgreSQL/JDBC (`type`), `nullable`, `precision`, `scale`.
- `rows` son arrays posicionales respecto de `columns`. Se ordenan en PostgreSQL
  por la PK en su orden declarado, también PK compuestas; no por orden de
  inserción. El clúster se inicializa con locale `C`.
- `NUMERIC`/`DECIMAL`: **strings** desde
  `ResultSet.getBigDecimal(...).toPlainString()`, conservando escala, signo y
  ceros. No hay paso por float/double ni notación exponencial. Los enteros son
  números JSON exactos; un futuro consumidor debe evitar convertir BIGINT a
  float/Number inseguro.
- SQL NULL se representa como JSON `null`, no vacío/cero/omisión. UUID permanece
  textual, sin regeneración ni mapeo de identidad. `timestamptz` se exporta como
  instante ISO-8601 **UTC (`Z`)**, con precisión disponible en PostgreSQL.
  Booleanos son booleanos y texto/barcodes conserva sus ceros iniciales.
- JSONB se representa como **string de JSON PostgreSQL**, con tipo `jsonb`
  explícito; no se parsean sus números a float ni se confunde un JSON null
  interno con SQL NULL. No se preserva whitespace original de un JSONB, que el
  propio source ya normaliza.
- Objetos con claves ordenadas, arrays estables, UTF-8, sin espacios y un LF
  final. El orquestador canoniza con `json.dumps(sort_keys=True,
ensure_ascii=False, separators=(",", ":"), allow_nan=False)`; su SHA-256 es el
  digest funcional comparado. No es una promesa de RFC 8785 para cualquier JSON.
  Tipos source desconocidos se rechazan; nunca se convierten silenciosamente.

Acciones explícitas:

| Tabla              | Acción           | Qué contiene                                                                                                                                                        |
| ------------------ | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `refresh_sessions` | `INVALIDATE`     | Metadata de columnas/PK y conteo source; `rows: []` deliberadamente. No se exportan identidades, familias ni token hashes reutilizables; cero sesiones importables. |
| `app_metadata`     | `REFERENCE_ONLY` | Fila técnica source como referencia, no instrucción para importar configuración operacional.                                                                        |
| Otras 35           | `EXPORT_ONLY`    | Filas source tipadas; exportar **no** autoriza importarlas durante el freeze.                                                                                       |

Por tanto, **37/37 pobladas en source no significa 37 tablas con filas
importables**: la omisión deliberada de sesiones se distingue de una tabla
source vacía con `source_count`. Las observaciones incluyen los 37 conteos.

## Dataset y evidencia

El seed toma las columnas/FKs/CHECKs de las migraciones source y el patrón de
inserción SQL explícita de
`backend/src/test/java/com/hiqnet/pos/FlywayMigrationPathTest.java` y
`DuplicateConstraintDbTest.java`. No pretende ejecutar los servicios de negocio
ni sustituye sus pruebas transaccionales/RBAC.

Datos incluidos:

- Dos organizaciones, tres branches (dos en A y una en B), tres usuarios,
  estados ACTIVE/INACTIVE, roles/grants organization-wide y branch membership.
  Se conserva el catálogo real de **59 permisos** sembrado por Flyway.
- Bootstrap completo, auditoría con JSONB y una sesión sintética solo contada
  para invalidación.
- Catálogo, precio monetario `900719925474.1234` cuyo coeficiente excede 2^53,
  precio cero, barcode con ceros, categoría opcional y timezone heredada NULL.
- Venta COMPLETED, turno CLOSED, pago CARD con tendered NULL, modificador
  negativo, receta ACTIVE con componente, efecto de inventario negativo y
  snapshots históricos distintos del catálogo actual. No se recalcula historia.
- Ledger OPENING → PURCHASE_RECEIPT → SALE_CONSUMPTION y balance exacto final
  `11.000000`; cantidades mínimas de seis decimales y `unit_locked=true`.
- Orden **ORDERED con `unit_cost IS NULL`**, otra RECEIVED con costo cero,
  cabecera/línea de recepción y movimiento enlazados. Mantener NULL distinto de
  `0.0000` es una aserción viva, no solo inspección del texto del fixture.

El Java verifica precisión/NULL/UUID/UTC en datos leídos vía JDBC, valida el
historial de **25 migraciones exitosas en orden**, llama `validate()` y confirma
que otro `migrate()` no ejecuta ninguna migración adicional. Exporta dos veces
dentro de la **misma transacción read-only REPEATABLE READ**. Después se destruye
ese clúster y se reconstruye otro desde cero con las mismas migraciones y seed.
Los timestamps funcionales y BCrypt son fijos. `installed_on`/tiempos de
ejecución Flyway quedan fuera del bundle y del historial proyectado.

Evidencias privadas:

| Archivo                                          | Contenido                                                                                                                            |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| `bundle-1.json`, `bundle-2.json`                 | Exportaciones canónicas funcionales de las dos instalaciones.                                                                        |
| `flyway-history-1.json`, `flyway-history-2.json` | Versiones/scripts/CRC32/check de éxito; sin timestamps ni duración técnica. CRC32 no es SHA-256.                                     |
| `manifest.json`                                  | SHA-256 de las 25 migraciones intactas, seed y Java; versión PostgreSQL y exclusiones. Sin filas ni credenciales.                    |
| `source-report.json`                             | Conteos por tabla, observaciones por instalación, repetición de snapshot, cleanup, comparación de dos digests y bloqueos explícitos. |

Si falla antes de exportar, el reporte marca fallo y `source_counts: null`
cuando aún no se obtuvieron, en lugar de afirmar cobertura cero o completa.
Si existen tablas sin filas, sus conteos y el bloqueo se conservan; no hay
false-green de cobertura. Los artefactos parciales de un fallo no son evidencia
completa.

## Pruebas

```sh
python3 -B -W error::ResourceWarning -m unittest discover \
  -s backend-rust/tools/migration-lab/fixtures/source -p 'test_*.py' -v

python3 -B backend-rust/tools/migration-lab/fixtures/source/native_cleanup_test.py \
  --pg-bin backend-rust/target/pg17-build/install/bin
```

La primera suite comprueba argumentos/URL, output exclusivo/padre existente,
0700/0600, ausencia de overwrite, serialización y uso de BigDecimal/snapshot,
entorno source-safe y timeout sin divulgar payload. La segunda utiliza clústeres
nativos propios e inyecta fallos en initdb, readiness, después de readiness y
`pg_ctl stop`, comprobando reaping/eliminación del temporal. Las aserciones JDBC
se ejecutan adicionalmente en cada instalación del comando principal.

Ejecución observada en este entorno (2026-10-27): Java 21.0.13, PostgreSQL
17.11; dos instalaciones completas V1–V25, **37/37 tablas source pobladas**, 166
filas source y **165 filas exportadas** (una sesión invalidada), ambos bundles
con el mismo digest. Ningún temporal de clúster quedó tras la ejecución. Esto
no convierte el dataset sintético en datos reales ni acredita los invariantes
que solo implementa la capa de aplicación.

## Integración posterior y criterios pendientes

`source-report.json` ya está integrado en el runner principal: `--java-source`
ejecuta este laboratorio, copia `source-report.json` y `source-manifest.json` a la
evidencia, y agrega `SOURCE-INSTALL-1`, `SOURCE-INSTALL-2` y
`SOURCE-RECONSTRUCTION` a los checks técnicos. Los bundles privados con hashes
BCrypt sintéticos **no** se copian a la evidencia ni se suben como artefacto.
La integración no cambia los gates normativos: PAR-08 se aprueba únicamente si
las dos instalaciones PASSED, 37/37 tablas y los digests coinciden; el resto de
PAR/AS sigue bloqueado. `source-report.json` conserva los conteos y la acción
`refresh_sessions: INVALIDATE`; no se infiere paridad de `status: PASSED`.

- **PAR-08 queda PASSED** en su definición exacta de F1–13: instalación limpia
  Flyway V1–V25, exportador determinista y mismo digest en ejecuciones iguales,
  probado por dos instalaciones independientes con 37/37 tablas. **No** acredita
  un round-trip Java → target ni el import target, que siguen BLOCKED.
  Si otra ejecución no completa las dos instalaciones, PAR-08 no se aprueba
  (el reporte lo marca BLOCKED y el runner no lo promueve).
- Import/restore/round-trip target y comparación completa, incluido manejo de
  conflictos físicos, siguen bloqueados por el freeze y fuera de alcance.
- `INVALIDATE` es una política de exportación, no una invalidación ejecutada en
  target. El futuro importador debe no restaurar ninguna sesión y exigir login.
- No hay aprobación de autorización/RBAC por API, carreras, reglas de servicio,
  upgrade con datos entre versiones, rollback/recovery ni matriz completa
  DDL13/PAR/AS. Flyway aquí recorre V1–V25 en una instalación vacía y el seed se
  aplica **después**, no ensaya backfills con negocio preexistente.
- La lista exacta de 37 tablas es intencional: una futura V26 exige una decisión
  explícita y revisar fixtures/contrato antes de ampliar este laboratorio.
