# Estrategia Rust B — preparación y revisión de la inversa

**2026-10-28 · PREPARACIÓN AUTORIZADA; EJECUCIÓN NO AUTORIZADA.**
La orden permite auditar y preparar B, no revertir, hacer merge, crear ramas o
commits, publicar, mover `dev`/`main` ni modificar el remoto o sus protecciones.
La revisión humana final y la autorización del PR de reversión están **pendientes**.
No se declara `dev` verde por esta preparación.

## 1. Alcance, autoridad y estado preservado

Leídos `AGENTS.md`, `CONTRIBUTING.md`, `git-workflow.md`, el paquete
[PRE0D-1](rust-0d-approval-package.md) completo, CC-0021 y las secciones de
estado/alcance de sus anexos D02, D03–D06 y D07. Esta revisión no acepta D01..D07,
ADR-0021, IMP-01/DDL13 ni cambios canónicos, importaciones o cutover.
La autorización de **preparar B** no es autorización de ejecutarla ni de configurar
protecciones. No se modifica el registro pendiente del paquete anterior.

Estado local comprobado, **sin fetch ni consultas remotas nuevas**:

| Ref                                                                                             | SHA observado                              |
| ----------------------------------------------------------------------------------------------- | ------------------------------------------ |
| `HEAD`, `origin/dev`, `feat/backend/rust-0d-readiness`, `feat/backend/rust-physical-resolution` | `329c0984722e8339f3e3b29497960d426ac2c43c` |
| Baseline pre-Rust; `dev` local                                                                  | `ebf7a30afc342a31d837a85772c5e85d0bc38efb` |
| `feat/backend/rust-foundation-migration-lab`                                                    | `6a15ee6a21ef80c17ab7276b337946cbcc4a4d37` |
| `main` local                                                                                    | `9b8b0acae2666fc48995c16d6344a0c0429bedf7` |
| `origin/main`                                                                                   | `2f0d6983325523a17a8a22ba8aa82e49b4994948` |

`origin/*` son refs de seguimiento locales, no una certificación del remoto ahora.
No se actualizó ninguna ref. El baseline auditado es `ebf7a30`, **no** `main` local
ni `origin/main`; sus diferencias no se deben incorporar a B.

El checkout inicial contenía siete archivos modificados y cuatro sin seguimiento
(PRE0D-1), sin diferencias staged. Se tomaron huellas de los **751 archivos
versionados o nuevos originales**, modos, bytes, refs, índice real y diffs staged/
unstaged. La comparación posterior a generar los artefactos fue idéntica.
La verificación repetible excluye únicamente este documento nuevo; los artefactos
están ignorados bajo `target/`. Las huellas no son copias de seguridad de PRE0D-1.

**Único archivo nuevo destinado a versionarse:** este documento. Excepción local
expresamente solicitada: artefactos de revisión en
`backend-rust/target/strategy-b-review/`, creado mediante la herramienta de
directorios antes de escribir. No se hizo `git add`, stash, checkout, reset, revert,
merge, commit, push ni cambios de permisos/procesos. La simulación solo escribe
un índice alternativo y los objetos Git necesarios para `write-tree`, sin refs.

## 2. Auditoría de los ocho commits

Rango **exclusivo del baseline e inclusivo del tip**: `ebf7a30..329c098`.
Los ocho son de un solo padre y forman una cadena continua sin merges.
Todos son alcanzables desde el `origin/dev` observado: no son trabajo por volver
a integrar mediante cherry-pick. El manifiesto conserva SHA completo, padre,
asunto y `name-status` por commit; `commits-audit.txt` añade `numstat`.
Esta es una auditoría del alcance de reversión, paths y CI, no una certificación
de corrección de todo el dominio Rust.

| Orden | SHA completo                               | Paths en ese commit | Cambio y efecto de B                                                                                                                                                                                                        |
| ----- | ------------------------------------------ | ------------------: | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | `4236fabb82c49f89f694301c4bfc4651bd74d7cf` |                  32 | Workspace/lock/toolchain, aplicación Local, core, SQLite y tres raíces SQLx iniciales; ADR-0020, paridad y guías. B retira la foundation y restaura guías compartidas. No modifica Flyway ni la aplicación Java.            |
| 2     | `7de03ed7c3e18a2b443eeb7b4b63a0a0d2131895` |                  34 | Migration lab, fixtures auth/source/SQLite/PostgreSQL, scripts, runner y documentación/ADR-0021. B los retira del árbol integrado, no de la historia. Los `.java` de fixtures viven en `backend-rust/`, no en `backend/`.   |
| 3     | `c995bf8f3bb80148ecddb42e06a33f209cc53c41` |                   4 | Detector, tests CI nuevos, workflow y `ci.md`: job Rust técnico, seguridad y gate normativo obligatorio. B restaura los dos archivos CI preexistentes y retira la suite añadida, junto con todo el experimento.             |
| 4     | `6567f877080502b40c27ad876a2203fc1c1d8d05` |                   3 | Evidencia 0b en `rust-foundation-pr.md`, `rust-foundation.md`, `rust-increment-0b-report.md`; sin funcionalidad nueva.                                                                                                      |
| 5     | `f120de86268fb85cfdb91f6c6b06fbf4ba5182d3` |                   3 | Referencias de commits en los mismos tres documentos. No debe tratarse como entrega funcional independiente.                                                                                                                |
| 6     | `6a15ee6a21ef80c17ab7276b337946cbcc4a4d37` |                   3 | Actualización final de referencias en esos tres documentos; ya integrada.                                                                                                                                                   |
| 7     | `43d9909da7d295fc56b7f3a7ad4907fbb04080ef` |                   4 | Verificador de digests/snapshots y ocho regresiones; añade ejecución de esa suite al workflow y su expectativa al test CI. B retira ese refuerzo porque retira el lab completo, no porque sus validaciones sean opcionales. |
| 8     | `329c0984722e8339f3e3b29497960d426ac2c43c` |                  13 | Resolución física propuesta, CC-0021, mapping compras, reconciliación Git e informes 0c; README Rust y antecedentes actualizados. B retira también estos documentos añadidos. No es aprobación del físico.                  |

No sumar los paths por commit como si fueran únicos. El delta acumulado contiene
**76 paths: 68 altas, 8 modificaciones, 0 bajas**; 14 746 líneas añadidas y 36
retiradas. La inversa elimina 68 archivos y restaura 8: 14 746 líneas retiradas y
36 añadidas. No hay renombres en este delta.

### Inventario del efecto de la inversa

| Área                                        | Paths únicos | Efecto                                                                                                                                                                  |
| ------------------------------------------- | -----------: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `backend-rust/`                             |           52 | Eliminar los archivos versionados de foundation/lab, lock, toolchain, tests, fixtures y tres migraciones SQLx; no ejecutar down ni borrar bases o artefactos ignorados. |
| `.github/`                                  |            3 | Restaurar detector/workflow y eliminar tests CI añadidos. Véase §3.                                                                                                     |
| `docs/`                                     |           19 | Eliminar 15 documentos Rust añadidos; restaurar 4 guías preexistentes.                                                                                                  |
| Raíz                                        |            2 | Restaurar `CONTRIBUTING.md` y `README.md`.                                                                                                                              |
| `backend/`, `frontend/`, Compose, `deploy/` |            0 | Ninguna modificación de contenido o modo.                                                                                                                               |

Los **ocho archivos restaurados** son exactamente:

```text
.github/scripts/ci-changes.sh
.github/workflows/ci.yml
CONTRIBUTING.md
README.md
docs/03-api/README.md
docs/05-development/ci.md
docs/05-development/coding-guidelines.md
docs/05-development/local-development.md
```

Los **68 archivos eliminados**, con rutas relativas a la raíz del repo:

```text
.github/scripts/test_ci_changes.py
backend-rust/Cargo.lock
backend-rust/Cargo.toml
backend-rust/README.md
backend-rust/apps/pos-local/Cargo.toml
backend-rust/apps/pos-local/src/config.rs
backend-rust/apps/pos-local/src/config_tests.rs
backend-rust/apps/pos-local/src/lib.rs
backend-rust/apps/pos-local/src/main.rs
backend-rust/apps/pos-local/src/shutdown.rs
backend-rust/apps/pos-local/src/shutdown_tests.rs
backend-rust/apps/pos-local/src/signals.rs
backend-rust/apps/pos-local/src/telemetry.rs
backend-rust/apps/pos-local/src/telemetry_tests.rs
backend-rust/apps/pos-local/tests/pool_cleanup.rs
backend-rust/apps/pos-local/tests/process_signals.rs
backend-rust/crates/pos-core/Cargo.toml
backend-rust/crates/pos-core/src/lib.rs
backend-rust/crates/pos-core/src/tests.rs
backend-rust/crates/pos-storage-sqlite/Cargo.toml
backend-rust/crates/pos-storage-sqlite/src/lib.rs
backend-rust/crates/pos-storage-sqlite/src/tests.rs
backend-rust/migrations/central/0001_operational_tenant.sql
backend-rust/migrations/control/0001_customer.sql
backend-rust/migrations/local/0001_organization_branch.sql
backend-rust/rust-toolchain.toml
backend-rust/tools/migration-lab/Cargo.toml
backend-rust/tools/migration-lab/README.md
backend-rust/tools/migration-lab/fixtures/auth/GenerateSpringVectors.java
backend-rust/tools/migration-lab/fixtures/auth/spring-vectors.json
backend-rust/tools/migration-lab/fixtures/migrations/0001_lab.sql
backend-rust/tools/migration-lab/fixtures/migrations/0002_lab_upgrade.sql
backend-rust/tools/migration-lab/fixtures/postgres/migrations/0001_lab.sql
backend-rust/tools/migration-lab/fixtures/postgres/migrations/0002_lab_upgrade.sql
backend-rust/tools/migration-lab/fixtures/postgres/transaction-failure/0003_lab_failure.sql
backend-rust/tools/migration-lab/fixtures/source/SourceFixture.java
backend-rust/tools/migration-lab/fixtures/source/native_cleanup_test.py
backend-rust/tools/migration-lab/fixtures/source/seed.sql
backend-rust/tools/migration-lab/fixtures/source/test_java_source_lab.py
backend-rust/tools/migration-lab/fixtures/transaction-failure/0003_lab_failure.sql
backend-rust/tools/migration-lab/scripts/build_postgres.sh
backend-rust/tools/migration-lab/scripts/java_source_lab.py
backend-rust/tools/migration-lab/scripts/test_verify_evidence.py
backend-rust/tools/migration-lab/scripts/verify_evidence.py
backend-rust/tools/migration-lab/src/auth.rs
backend-rust/tools/migration-lab/src/cli.rs
backend-rust/tools/migration-lab/src/main.rs
backend-rust/tools/migration-lab/src/postgres.rs
backend-rust/tools/migration-lab/src/provenance.rs
backend-rust/tools/migration-lab/src/report.rs
backend-rust/tools/migration-lab/src/source.rs
backend-rust/tools/migration-lab/src/sqlite.rs
backend-rust/tools/migration-lab/tests/runner.rs
docs/01-architecture/adr/0020-rust-foundation-sqlx-migration-boundary.md
docs/01-architecture/adr/0021-frozen-physical-parity-conflicts.md
docs/05-development/rust-adr0021-change-control.md
docs/05-development/rust-auth-lab.md
docs/05-development/rust-foundation-pr.md
docs/05-development/rust-foundation.md
docs/05-development/rust-increment-0b-report.md
docs/05-development/rust-increment-0c-git.md
docs/05-development/rust-increment-0c-report.md
docs/05-development/rust-missing-physical-proposal.md
docs/05-development/rust-parity-baseline.md
docs/05-development/rust-physical-audit.md
docs/05-development/rust-postgres-lab.md
docs/05-development/rust-purchase-parity-contract.md
docs/05-development/rust-source-lab.md
```

El manifiesto identifica contenido y modo anterior/posterior con OID Git y SHA-256
por blob. Las guías restauradas pierden sus referencias Rust, no cambia ningún
endpoint/DTO/permiso/ApiError. El único delta de `docs/03-api/` es su README humano,
no OpenAPI. La restauración exacta también recupera la etiqueta antigua **V1–V21**
en el diagrama de `local-development.md`, aunque las 25 migraciones reales siguen
intactas: discrepancia editorial conocida para la revisión final, no cambio SQL.
No se corrige aquí para no ampliar el write set ni falsear la igualdad de árboles.

## 3. Auditoría del cambio CI y riesgo explícito

Se comparó `.github/` completo (`ci-forward.diff`) y se parsearon los YAML base/tip.
Los objetos completos de los jobs `backend`, `frontend`, `compose`, `e2e`, y la
configuración global del workflow son **idénticos**. Esto es comprobación estática,
no ejecución de esos jobs.

- **Detector:** B retira `backend-rust/` de rutas conocidas y las salidas
  `rust`/`rust_run`. Conserva las rutas Java/React/Compose, docs-only, fallback
  completo ante rutas desconocidas/base inválida y ejecución completa manual.
  Por su lógica, un PR que cambie `.github/` como esta inversa activa
  Java/React/Compose; esto aún debe confirmarse en el run del candidato.
- **Suite CI:** elimina `.github/scripts/test_ci_changes.py` y el step que la
  ejecuta. Se pierden sus **21 tests**, incluidos casos de áreas legacy, detección
  fallida y gates, no solo casos específicos Rust. No se ejecutó esa suite aquí.
- **Job Rust:** elimina fmt/clippy/test/build, toolchain 1.99.1, Java para el lab,
  PostgreSQL 17.11 compilado y fijado por digest, Gitleaks 8.24.3 verificado,
  cargo-audit 0.22.2, verificador de evidencia y artefactos. También elimina el
  step bloqueante `--require-complete`, su evaluación `always()` y el verificador
  del bloqueo esperado. No se conserva un esquema Rust integrado sin su gate:
  **también se retira todo su código, SQLx y lab**.
- **`CI Gate`:** conserva nombre estable, `always()` y rechazo de failure/cancelled
  entre sus dependencias legacy; retira `rust` de `needs`, la validación de
  `rust_run`, la exigencia de Rust success y la exigencia explícita de
  `changes.result == success`. El gate del baseline acepta `skipped` en general,
  incluso detección omitida. Es una pérdida de endurecimiento transversal que
  debe aceptar expresamente el revisor, no ocultarse como un mero cambio Rust.
- **E2E:** misma condición en ambas versiones: solo push a `main` o
  `workflow_dispatch`; PR a `dev` no lo ejecuta. No promover a `main` para probar
  B. Tras autorización separada, usar el mecanismo manual existente desde `main`
  con `target_sha` completo del candidato revisado, sin mover esa rama.

La propuesta B auditada es la **inversa exacta**, no una edición selectiva de CI
para aprobar el mismo esquema parcial. Si se decide conservar tests genéricos o
el endurecimiento transversal, preparar y revisar un cambio explícito adicional:
ese candidato ya no tendrá el árbol exacto del baseline y necesita nueva prueba,
manifest y autorización. No se adoptó esa variante aquí.

## 4. Artefactos, comandos y prueba aislada

Directorio local ignorado: `backend-rust/target/strategy-b-review/`.

| Artefacto                                                             | Función                                                                                                                                               |
| --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `inverse.patch`                                                       | Captura binaria exacta de `git diff --binary origin/dev ebf7a30`; inversa del cambio publicado, no del PRE0D-1 dirty.                                 |
| `manifest.json`                                                       | SHAs/árboles, ocho commits, 76 paths con modos/OIDs/digests, ámbitos protegidos, comandos/checks y digests de artefactos.                             |
| `virtual.index`                                                       | Índice alternativo cargado desde `origin/dev` y transformado solo en memoria/índice. No es el índice real.                                            |
| `initial-state.json`, `initial-status.txt`, `preservation-check.json` | Huellas originales y comprobación de preservación; no almacenan cuerpos de los cambios locales.                                                       |
| `virtual-index-checks.json`                                           | Comandos apply/read-tree, salidas y exit codes observados.                                                                                            |
| `ci-forward.diff`, `ci-structure-checks.json`                         | Diff CI y comparaciones estáticas de YAML/jobs/gates.                                                                                                 |
| `commits-audit.txt`, `inverse-stat.txt`                               | Auditoría por commit y tamaño del delta inverso.                                                                                                      |
| `prepare_review.py`                                                   | Procedimiento reproducible; rechaza sobrescribir evidencia y generar de nuevo si este documento ya existe. Modo `--verify-preservation` solo lectura. |

Huellas del lote generado:

| Objeto                                        | SHA-256                                                            |
| --------------------------------------------- | ------------------------------------------------------------------ |
| `inverse.patch` — **864 353 bytes**           | `fa89970e12fdb71c073c6eceae361ea426c07db4f3423326ade1a1075f505295` |
| `manifest.json`                               | `9cfd17c849f4edffe87f0cfb917ca86fa6a6a93a20c8d587a35e401f5c6cd8da` |
| Índice real, antes/después                    | `a936d96ea30b0b005c50d5f86c58896eddab39e6b107252d934986d289e4709f` |
| Salida completa `git show-ref`, antes/después | `9149a48a79df272756585084d4f69c350bc8ae2e309b93b2337d1dfc5bcabf44` |

Son huellas de integridad local, **no firmas ni aprobación**. `target/` no se
versiona; antes de un PR futuro deberá acordarse cómo adjuntar/conservar estos
artefactos fuera del output descartable. No usar `git add -f target/`.

### Comandos realmente ejecutados

Desde la raíz se ejecutó:

```sh
python3 -B backend-rust/target/strategy-b-review/prepare_review.py
```

El script capturó stdout binario mediante `subprocess`, sin redirección sobre
archivos versionados. Comprobó HEAD/ref/rama, ocho padres lineales y estado
original. La secuencia Git interna siguiente se ejecutó **siempre** con
`GIT_INDEX_FILE=/home/grxson/Work/HiQNet/point-of-sale/backend-rust/target/strategy-b-review/virtual.index`
y `GIT_OPTIONAL_LOCKS=0` en el entorno de cada subprocess:

```text
git read-tree origin/dev
git apply --cached --check /home/grxson/Work/HiQNet/point-of-sale/backend-rust/target/strategy-b-review/inverse.patch
git apply --cached /home/grxson/Work/HiQNet/point-of-sale/backend-rust/target/strategy-b-review/inverse.patch
git write-tree
git diff --cached ebf7a30afc342a31d837a85772c5e85d0bc38efb --
git apply --cached --reverse --check /home/grxson/Work/HiQNet/point-of-sale/backend-rust/target/strategy-b-review/inverse.patch
```

**No copiar esa secuencia sin el entorno alternativo.** El índice temporal debía
no existir antes de `read-tree`; no se inicializó sobre el índice real.
`--cached` evita leer/modificar archivos del checkout. La comprobación inicial
usa la inversa **sin `--reverse`** sobre el tip. La última comprobación usa
`--reverse` sobre el resultado ya restaurado y solo verifica que se podría
reintroducir; no la aplica. No se hizo `git apply --reverse --check` sobre el
worktree dirty: comprobaría la dirección/estado equivocados y no aislaría PRE0D-1.

### Resultado probado

| Árbol Git                                        | OID                                        |
| ------------------------------------------------ | ------------------------------------------ |
| Tip `329c098^{tree}`                             | `238fad6c0a906dcf0be64d63d78057e510ce030b` |
| Baseline `ebf7a30^{tree}`                        | `1ed355d8bbfd1e5e39ab495a66919cf8b72ff7ce` |
| `write-tree` del índice temporal tras la inversa | `1ed355d8bbfd1e5e39ab495a66919cf8b72ff7ce` |

**Igualdad exacta de árbol**, incluidos contenido, modos y paths;
`git diff --cached baseline` quedó vacío. Además se comprobaron OIDs idénticos
base/tip/virtual para `backend/`, `frontend/`, `docker-compose.yml`, `deploy/`, y
las **25 migraciones Flyway V1–V25** por path/entrada. No se ejecutó Flyway ni se
conectó ninguna base. Las migraciones SQLx retiradas son las nuevas raíces Rust,
no el historial Java. La prueba corresponde al parche publicado aislado:
**no** compara el worktree dirty con el baseline, ni incluye este documento o
los cuatro anexos nuevos. Un PR que conserve documentación de revisión deberá
listar esa excepción documental y revisar su delta aparte.

Comandos seguros para reinspección del lote ya preparado, desde la raíz:

```sh
python3 -B backend-rust/target/strategy-b-review/prepare_review.py --verify-preservation
sha256sum backend-rust/target/strategy-b-review/inverse.patch backend-rust/target/strategy-b-review/manifest.json
git --no-optional-locks status --short --branch
git --no-pager diff --name-status
git --no-pager diff --cached --name-status
```

El verificador contrasta de nuevo archivos originales, refs, índice real y
artefactos, excluyendo solo este documento autorizado. No aplicar el parche al
checkout ni al índice real como parte de la reinspección.

## 5. PRE0D-1: conflictos que impiden ejecutar sobre este checkout

Los siete archivos dirty están **todos** dentro de los paths de la inversa:

| Path local modificado                                               | Acción de la inversa publicada |
| ------------------------------------------------------------------- | ------------------------------ |
| `backend-rust/README.md`                                            | Eliminar                       |
| `backend-rust/tools/migration-lab/scripts/test_verify_evidence.py`  | Eliminar                       |
| `backend-rust/tools/migration-lab/scripts/verify_evidence.py`       | Eliminar                       |
| `docs/01-architecture/adr/0021-frozen-physical-parity-conflicts.md` | Eliminar                       |
| `docs/05-development/ci.md`                                         | Restaurar baseline             |
| `docs/05-development/rust-adr0021-change-control.md`                | Eliminar                       |
| `docs/05-development/rust-increment-0c-report.md`                   | Eliminar                       |

Los cuatro nuevos no están en ninguno de los ocho commits ni en el parche:
`rust-0d-approval-package.md`, `rust-d02-approval-contract.md`,
`rust-d07-approval-contract.md`, `rust-valuation-approval-contract.md`, todos en
`docs/05-development/`. Quedaron intactos. **Conservar solo refs no preserva estos
once cambios locales**; los anexos además quedarían apuntando a documentos
retirados si se mezclaran indiscriminadamente en la línea Java restaurada.

Antes de cualquier ejecución futura: acordar y autorizar un checkpoint/registro
persistente de PRE0D-1 en la línea de trabajo Rust, comprobar su contenido y
obtener checkout limpio sin descartar nada. No se hizo stash, commit o cambio de
rama para resolverlo ahora. No suponer que los digests son una copia recuperable.

## 6. Plan futuro condicionado — NO ejecutado

1. **Revisión independiente del lote:** titular/revisor valida los ocho SHAs,
   inventario, pérdida de endurecimiento CI, tratamiento documental y preservación
   completa de PRE0D-1. Autorizar separadamente ejecución, PR/publicación y
   protecciones; no inferir ninguna de esta preparación.
2. **Revalidar base antes de operar:** con permiso, consultar remoto y comprobar
   que `origin/dev` sigue en el tip auditado. Si avanzó, detener esta receta y
   evaluar cambios de terceros/conflictos; regenerar pruebas/manifiesto. No usar
   el `dev` local atrasado como base ni moverlo para simular una reversión.
3. **Checkout limpio y rama short-lived desde el dev vigente**, tras preservar
   PRE0D-1 y las refs Rust existentes. Revertir los ocho en orden inverso:
   `329c098`, `43d9909`, `6a15ee6`, `f120de8`, `6567f87`, `c995bf8`, `7de03ed`,
   `4236fab`. El parche acumulado auditado es un oráculo del resultado esperado,
   no autorización para aplicarlo en el checkout actual. Registrar un propósito
   coherente y un PR hacia `dev`; nunca reset/rebase/force-push de historia publicada.
4. **Validar el SHA real del candidato y su integración con la base:** comparar
   árbol con baseline (o baseline más excepción documental explícita), inspeccionar
   todo CI/diff y ejecutar Java, React, Compose y E2E. Adjuntar resultados del SHA
   exacto y `CI Gate` verde; E2E skipped en PR no satisface esta revisión.
5. **Solo después de revisión/aprobación y autorización de merge**, integrar por
   PR según la convención de squash. Registrar SHA real de la reversión integrada,
   enlace de PR y CI. No promover a `main` sin Release Audit y autorización propias.

### Reintroducción reversible sin duplicar los ocho commits

Los ocho commits permanecen en la historia aunque su contenido desaparezca del
árbol. Las refs Rust observadas ya los retienen; no se creó una rama adicional.
Antes de su limpieza futura, asegurar conservación deliberada del trabajo Rust y
del checkpoint PRE0D-1. Revertir contenido no revoca por sí mismo ningún despliegue
ni modifica datos: este plan no es rollback de producción.

Cuando se cierren aprobaciones y gates, partir del `origin/dev` **vigente** en una
nueva rama short-lived y **revertir la reversión integrada** por un PR nuevo. Si
se integró mediante squash, el objetivo es el SHA de ese squash, no los SHAs
locales intermedios; si quedó más de un commit de reversión, auditar y deshacer
ese conjunto en orden inverso. Determinar esos SHAs entonces, no inventarlos hoy.

Añadir a ese candidato los avances Rust revisados, incluido PRE0D-1 cuando esté
registrado, resolver conflictos con cambios posteriores y restaurar conjuntamente
workspace, SQLx, documentación, seguridad, tests y gate normativo completo.
No cherry-pickear otra vez `4236fab..329c098`, ni esperar que un merge de una rama
que solo contiene esos ancestros restaure cambios revertidos. No duplicar commits
ni aplicar inversa + cherry-picks para el mismo contenido. El PR completo debe
pasar gates del **nuevo SHA**, con evidencias y revisión independiente; no
eliminar `--require-complete`, redefinir PASS, omitir escenarios o usar docs-only
verde para convalidar el experimento. ADR/CC/canónicos y autorización productiva
continúan siendo decisiones distintas.

## 7. Protecciones existentes en el expediente y propuestas pendientes

El paquete PRE0D-1 §6 registra lecturas previas: protection de dev/main con 404
explícito `Branch not protected`, reglas activas `[]`, run
[40775256216](https://github.com/HiQNet-Dev/point-of-sale/actions/runs/40775256216)
de `329c098` fallido por gate Rust/`CI Gate`, Java/React/Compose success y E2E
skipped. En esta sesión se repitió `git fetch` y se consultó por API el run más
reciente de `dev`: sigue siendo ese mismo run `failure` sobre `329c098`. No se
releyeron las protecciones en esta sesión, no se modificó el remoto y no se
atribuyen personas/bypass.

Se mantienen las propuestas ya existentes, **sin aplicarlas**:

- Rulesets activos separados para `dev`/`main`, PR obligatorio, al menos una
  aprobación humana independiente, invalidar aprobación al cambiar el diff y
  exigir aprobación del último push por otra persona; sin bypass rutinario de
  admins/apps.
- `CI Gate` requerido con emisor GitHub Actions (app **15368**, según expediente),
  rama al día, conversaciones resueltas, bloqueo de force-push y borrado. El nombre
  estable sigue presente en baseline. Su configuración remota no reemplaza la
  auditoría de qué exige el job: véase el debilitamiento transversal de §3.
- Política release/hotfix de `main`, excepciones de emergencia acotadas y auditadas
  con titular+revisor, PR, SHA/motivo/plazo/rollback y post-review; no otorgar bypass
  permanente al agente.
- Verificar después de autorización con GET protection/rules y PR controlado
  rojo/sin review; no ensayar un push no autorizado. No activar merge queue sin
  soporte `merge_group` ni exigir historia lineal global que contradiga releases.

No se consultó ni modificó la configuración de protección en esta tarea. No hay
autorización implícita para configurar estas protecciones por aprobar la
preparación B.

## 8. Pruebas realizadas frente a pendientes y salida de revisión

| Comprobación                                                                       | Resultado y alcance                                                                                                                                                                                                        |
| ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HEAD/refs/rama, ocho commits y padres                                              | **OK local**, valores fijados en §1–2.                                                                                                                                                                                     |
| Generación binaria y hashes                                                        | **OK**, 76 paths/68 eliminaciones/8 restauraciones; artefactos ignorados.                                                                                                                                                  |
| Apply check + apply con índice alternativo                                         | **Exit 0**; nunca índice real/worktree. Reverse check posterior también exit 0.                                                                                                                                            |
| Árbol virtual = baseline y diff cached vacío                                       | **OK**, OID exacto de §4. No implica suites aprobadas.                                                                                                                                                                     |
| Java/React/Compose/deploy/Flyway intactos                                          | **OK por árboles/entradas**, incluidas 25 migraciones; no ejecución de migraciones.                                                                                                                                        |
| YAML base/tip y jobs legacy                                                        | **OK estático**; detector `bash -n` en ambas versiones exit 0. No valida el servicio GitHub Actions.                                                                                                                       |
| Preservación                                                                       | **OK al generar**: 751 archivos, refs e índice real idénticos. Tras añadir documentación autorizada, `--verify-preservation` informa discrepancia **esperada**; los SHA-256 del parche/manifiesto siguen siendo los de §4. |
| Java                                                                               | **PENDIENTE para candidato B**: `./mvnw -B -ntp clean verify` desde `backend/`. No ejecutado aquí.                                                                                                                         |
| React                                                                              | **PENDIENTE para candidato B**: instalación con lock, lint, typecheck, test y build con pnpm desde `frontend/`. No ejecutado aquí.                                                                                         |
| Compose                                                                            | **PENDIENTE para candidato B**: `docker compose config`. No ejecutado aquí.                                                                                                                                                |
| E2E Chromium + PostgreSQL                                                          | **PENDIENTE para candidato B**, vía mecanismo autorizado de SHA fijado; no ejecutado ni despachado aquí.                                                                                                                   |
| Suites Rust, 17 regresiones locales PRE0D-1, 21 regresiones CI, lab, audit/secrets | **NO REEJECUTADAS**. Sus resultados anteriores pertenecen a PRE0D-1/CI base, no a esta preparación ni a un candidato B nuevo.                                                                                              |
| CI remota, PR/merge, enforcement                                                   | **NO ejecutados / PENDIENTES**; nada publicado ni configurado.                                                                                                                                                             |

**Salida:** inversa y manifest preparados y verificados de forma aislada;
PRE0D-1 preservado. **Falta revisión final humana** del alcance, CI y documentación,
preservación persistente del trabajo local antes de operar, autorizaciones
separadas y suites del futuro candidato. B puede retirar coherentemente el
experimento del árbol de integración; no se afirma que ya lo haya retirado ni
que el estado resultante haya pasado Java/React/Compose/E2E.
