# Incremento 2C — avance de seguridad del camino persistente

**Estado: parcial; no se declara completada la activación de producto.**

Base verificada: `0e7afa3251a229a6b6882d9a60c3ef22c868c3e9`, rama
`feat/fullstack/rust-desktop-vertical-1x`. Checkout inicialmente limpio, un solo
worktree. PR #84 abierto, Draft hacia `dev`, mismo SHA remoto y sin checks
asociados en `statusCheckRollup` al consultar GitHub. No se ejecutaron Actions,
merges, releases ni promociones de migraciones. Las correcciones inicialmente
sin commit se consolidan en esta continuación mediante commits enfocados con
`[skip actions]`; ambos workflows usan eventos compatibles con esa omisión nativa
de GitHub. `VERSION` no cambia. La publicación no equivale a CI aprobado.

## 1. Funcionalidad corregida, sin otro laboratorio

### Historial SQLx en el camino de producto

La reapertura devolvía éxito sin invocar el migrador cuando existía historial.
Se reprodujo el defecto con SQLite real: tras alterar el checksum de `0001`,
`prepare_installation` devolvía `REOPENED` en lugar de rechazar la base.

Ahora `pos-application::installation::prepare_installation`:

1. Observa el historial y rechaza esquemas futuros antes de migrar.
2. Invoca siempre el migrador **aprobado**, incluso al reabrir, para validar
   checksums e historial SQLx y aplicar solo upgrades aprobados pendientes.
3. Vuelve a observar y exige alcanzar la versión aprobada antes de devolver éxito.
4. Conserva en el reporte la clasificación del momento de apertura.

El adaptador sigue ejecutando exclusivamente `migrations/local`; no se cambió
ningún SQL ni checksum histórico. No se introdujo SQL en Application o Tauri.
La comprobación del historial no acredita por sí sola que nadie haya alterado
manualmente las tablas de negocio.

Pruebas añadidas: checksum alterado, migración fallida, upgrade aprobado pendiente,
migrador que no alcanza la versión esperada y rechazo de esquema futuro antes de
invocar al migrador. El composition root desktop también verifica rechazo por
checksum y liberación del candado después de un fallo de arranque.

### Apertura de directorios y exclusión desktop

`apps/pos-desktop::installation_dir` añade:

- Rechazo de enlaces/junctions/puntos de reanálisis en todos los antecesores.
- Validación del archivo principal, `-wal`, `-shm`, `-journal` y lock file;
  rechaza redirecciones y objetos que no sean archivos regulares.
- Directorio final privado en Unix: rechaza permisos de grupo/otros sin cambiar
  silenciosamente los permisos de una instalación existente.
- Candado exclusivo del SO mediante `File::try_lock` sobre `.hiqnet-pos.lock`,
  adquirido antes de abrir SQLite. La segunda instancia desktop recibe un fallo
  seguro de arranque. El estado mantiene el descriptor hasta cerrar el pool.
- El lock file no se elimina: borrarlo permitiría bloquear objetos distintos bajo
  el mismo nombre. El SO libera el candado al cerrar el descriptor/proceso.

No hay rutas recibidas desde JavaScript ni cambios de comandos/bindings IPC.
El candado es cooperativo y solo lo usa desktop: no impide que herramientas o
`pos-local` que no participen en el protocolo abran el mismo archivo. SQLite
conserva la responsabilidad de las transacciones y `BEGIN IMMEDIATE`.

## 2. Validación ejecutada

| Control | Resultado de este avance |
| --- | --- |
| Headless: fmt, Clippy estricto, build con workspace/all-features/locked | PASS |
| Headless: `cargo test --locked --workspace --all-features` | **315 PASS**, 0 fallos; incluye arquitectura y SQLite |
| Desktop Windows: fmt, Clippy estricto, test, build | **29 PASS**, 0 fallos |
| Directorios Linux Docker, usuario de la imagen de pruebas y sin red | **9 PASS**, 0 fallos |
| Frontend: lint y typecheck | PASS |
| Frontend: suite | **212 PASS**, 3 skipped |
| Contratos Rust↔TypeScript | **3 PASS** |
| Frontend build | PASS; warning existente de chunk mayor de 500 kB |
| E2E real React/WebView2/IPC: laboratorio | PASS; `productive: false` |
| E2E real React/WebView2/IPC: instalación persistente | PASS; `authEnabled: false`, `productive: false` |
| Docker build/runtime y smoke de instalación/reinicio | PASS; no-root, permisos `700 10001:10001`, healthcheck y cierre limpio |
| `git diff --check` | PASS al revisar los cambios de código |

El smoke Docker conservó el digest del archivo cerrado entre arranques:
`2477458c136582bf068f92929061618811b9a59c9c8913040c9f7e2686a2e03b`.

La prueba Linux del módulo de directorios compila **el archivo fuente actual**
montado de solo lectura con `rustc --test`, usando la dependencia `tempfile` ya
compilada en `hiqnet-pos-rust-contracts:local`. Ejecuta las ramas Unix de enlaces,
permisos, auxiliares y candado. No equivale a compilar/probar todo Tauri en Linux.
El E2E gráfico se ejecutó en Windows; Docker headless no se presenta como E2E.

Fallos intermedios documentados:

- La regresión del checksum falló antes del arreglo y pasó después.
- Las primeras pruebas Unix usaban directamente un directorio temporal que no
  cumplía la política privada; se corrigieron las fixtures para crear el
  subdirectorio mediante el validador real, sin debilitar la política.
- Una repetición de la suite headless agotó 120 s. La ejecución completa posterior
  con techo de 300 s terminó con las 315 pruebas aprobadas. Log local (ignorado
  por Git): `backend-rust/target/2c-workspace-tests.log`.

## 3. Autorizaciones y activación

| Bloque | Estado | Activado en este avance |
| --- | --- | --- |
| A — identidad/sesión/RBAC/auditoría/bootstrap | Pendiente de aceptación, reconciliación SYSTEM/USER y política de credenciales | No |
| B — catálogo/cajas/turnos | Pendiente; depende de identidad autorizada | No |
| C — ventas/inventario/checkout | Pendiente y sujeto a dependencias físicas | No |
| D — transporte/almacenamiento | Diseño aceptado el 2026-10-30 | Se mejora apertura segura y exclusión desktop; no distribución |
| CC21-03 | Aceptación parcial del costo pactado opcional | Se conserva; no se amplía su alcance |
| D-AUTH-09 | Bloqueada | No se activa login ni política LAB comercial |

Solo `0001_organization_branch.sql` permanece en `migrations/local`. `0002`–`0016`
siguen como propuestas; no hay nuevas autorizaciones ni migraciones promovidas.

D-AUTH-09 conserva el benchmark/interoperabilidad y admisión acotada de 2B. Este
avance **no** añade medición sobre hardware objetivo, política final de intentos
por cuenta/instalación, enfriamiento ni aprobación criptográfica. No se anuncia
login comercial, sesión auténtica ni bootstrap canónico terminado.

## 4. Purchasing y paridad

Las 17 operaciones de Purchasing de 2B permanecen implementadas en Application y
SQLite candidato; sus pruebas se ejecutaron dentro de las 315 headless. **No se
añadieron handlers, DTOs/bindings o pantallas Purchasing en este avance.** Tampoco
se ejecutó una comparación diferencial nueva contra los 17 endpoints Java.

El modo instalación sigue exponiendo únicamente `installation_status`; las
lecturas de sucursales/categorías/productos y la escritura de producto siguen
restringidas al modo sintético. Ninguna operación comercial nueva se presenta
como disponible. Se actualiza la guía IPC con los fallos de arranque y sus límites.

No hay evidencia nueva para cambiar los 30 casos normativos. El último estado
reportado se conserva como antecedente: **1 PASSED, 28 BLOCKED, 1 NOT_RUN**. No se
revalidó el conjunto normativo en este avance ni se modificaron sus estados,
`--require-complete`, DDL13/PAR/AS o CI Gate.

## 5. Trabajo restante verificable

1. Bloque A: cerrar técnicamente SYSTEM/USER y bootstrap, y completar la política
   de intentos/expiración/rehash/revocación sin dos caminos comerciales de identidad.
2. D-AUTH-09: medición en hardware representativo y aceptación explícita antes de
   usar parámetros comerciales. El contenedor limitado no sustituye ese hardware.
3. Bloque D: ACL Windows real, mitigación de TOCTOU/aperturas atómicas, política de
   almacenamiento compartido, APIs nativas Tauri, identificador de bundle definitivo
   con continuidad de datos, backup/restore y actualizaciones seguras. No se mueve
   ninguna base ni se fija silenciosamente el identificador comercial.
4. Preparar IPC/React de Purchasing sobre los casos existentes, con disponibilidad
   cerrada en producto hasta resolver `0016` y la identidad. Recepción económica,
   fichas físicas, snapshots/import y ADR-0021 siguen pendientes.
5. Tras autorización expresa A/B/C, promover solo el conjunto aprobado y ejecutar
   clean install/upgrade, checksums, rollback, aislamiento y reapertura antes de
   habilitar operaciones comerciales. No activar por una bandera que cargue proposals.
6. Completar contratos, pantallas, pruebas diferenciales e importador versionado
   con distribución Local/Central/Control aprobada. El E2E productivo completo aún
   no puede declararse ejecutado.
7. Antes de integrar a `dev`: autorización para CI, checks completos sobre el SHA
   final y gates/aprobaciones exigidos. Las pruebas locales no reemplazan CI.
8. Antes de retirar Java/v0.4.0: paridad completa, Purchasing expuesto, identidad y
   esquema productivos, instalador, recuperación, plan de reversión y Release Audit.

No hubo subagentes disponibles en las herramientas de esta sesión. Se utilizaron
las skills instaladas de arquitectura, debugging, testing, documentación y revisión
de código mediante lectura de sus instrucciones. La revisión del diff fue propia,
no una revisión independiente por un segundo agente. No había skill/herramienta
`secure-coding` disponible y no se atribuye una auditoría externa inexistente.
