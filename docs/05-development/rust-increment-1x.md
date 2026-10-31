# Incremento 1X — primera vertical React / Tauri / SQLite

## Punto de partida y alcance

Se verificaron #83, el checkout limpio y `b81f2800c12a6ae2a077cc3f2fad16024b225815`
local **y remoto**. El commit Docker no se descartó ni se reescribió. La rama de
trabajo es `feat/fullstack/rust-desktop-vertical-1x`.

Implementado: tres lecturas administrativas invocables desde una pantalla React
real dentro de Tauri v2. No se implementó un segundo Sale ni se copiaron reglas
de checkout; `pos-lab` se extrajo mecánicamente a biblioteca + CLI para compartir
el escenario sintético. No hay HTTP Rust ni cutover Java.

## Código y frontera

- `tools/desktop-lab`: composition root aislado, handlers, DTOs, capabilities y
  registro compartido con generación Tauri Specta.
- `organization::BranchReader` y `list_authorized_branches`: puerto y consulta
  de Application. SQLite implementa la lectura con su adaptador ya existente.
  Un principal de sucursal no puede enumerar toda la organización.
- `desktop.generated.ts` y `desktop-client.ts`: bindings generados y servicio
  frontend, sin invocaciones dispersas en componentes.
- `DesktopCatalogPage`: sucursales, categorías, productos, navegación de ventana
  por offset y actualización; entrada `rust-lab.html` separada de Java.
- `smoke-local.mjs`: runtime headless, proyecto/volumen nuevos y limpieza limitada
  a sus recursos, health y persistencia comparada después del checkpoint.

Contratos, versiones, comandos de ejecución y errores:
[guía IPC](../03-api/rust-desktop-ipc.md). Decisión de aislamiento y aceptación
pendiente: [ADR-0023](../01-architecture/adr/0023-tauri-readonly-laboratory.md).

## Seguridad y activación

Los handlers solo delegan. No aceptan actor, organización, sesión ni ruta SQLite
como parámetros. Cada llamada resuelve de nuevo sesión/principal/roles/permisos
DB-authoritative. Se prueban revocación, expiración, retirada de permiso,
organización ajena, principal scoped a sucursal, overposting y origen remoto.
No hay bootstrap ni comandos de escritura IPC.

La sesión y el reloj son fixtures de laboratorio, no autenticación comercial.
No se habilita el adaptador en `pos-local`, no se promociona ninguna propuesta
y no se unifican los caminos de mutación Organization/Branch sin resolver SYSTEM.
El expediente canónico continúa PENDIENTE, con aceptación vacía y D-AUTH-09
bloqueada por benchmark/interoperabilidad. También siguen pendientes el contrato
físico completo, ADR-0021/0022 y las evidencias DDL13/PAR/AS requeridas para merge.

## Evidencia ejecutada

- Windows workspace headless: fmt, Clippy `-D warnings`, tests y build PASS;
  **279 tests aprobados**, 0 fallidos. Log local ignorado:
  `backend-rust/target/1x-workspace-tests.log`.
- Workspace gráfico independiente: fmt, Clippy, **8 tests aprobados** y build
  ejecutados aparte; no se infiere su validación del workspace headless.
- Frontend: lint, typecheck, build y **211 tests aprobados**; tres pruebas Cargo
  opt-in omitidas por la suite normal y ejecutadas aparte: **3/3 PASS**.
- E2E Windows WebView2 **real**: PASS. React muestra sucursal, categoría y producto,
  conserva `12.3456`, actualiza y rechaza página inválida/overposting.
  El cierre normal de la ventana también se verifica. No es mockIPC.
- Docker Linux headless: build y suite completa PASS; **288 tests Rust aprobados**,
  211 frontend y 3 integraciones Rust/TypeScript. Incluye concurrencia,
  rollback, CAS, checkout/inventario, auditoría y reapertura de las suites existentes.
  Log local ignorado: `backend-rust/target/1x-docker-contracts.log`. Una repetición
  final combinada build+suite superó el timeout de 300 s: el log registró los
  288 tests Rust y 211 frontend aprobados, pero no llegó al marcador final. Las
  tres integraciones y el build frontend restantes se ejecutaron después como
  comando Docker focalizado, con exit 0. No se atribuye exit 0 al comando vencido.
- Smoke Docker de `pos-local`: base vacía, migraciones autorizadas, health,
  directorio `700 10001:10001`, cierre limpio, reinicio y hash de archivo cerrado
  idéntico: `2eafcc40111ec86b5b8807d56c7df765c5ed0ab8695b910b7e2e1cf3279ce89c`.
  Sus contenedores y volúmenes temporales se eliminaron; no se tocaron volúmenes
  existentes. El hash con WAL abierto no se usa como evidencia de persistencia.
- Vite mantiene una advertencia no bloqueante de tamaño del bundle Java.
- No se ejecutaron desktop Linux, instalador, PostgreSQL 17 Migration Lab completo
  ni paridad diferencial completa en este incremento. Docker no sustituye GUI.

El primer build paralelo Docker devolvió `context canceled` sin causa concluyente.
Se excluyeron targets anidados del nuevo workspace gráfico para evitar enviar
artefactos locales; los builds posteriores separados pasaron. La primera ejecución `exec` desde Git Bash convirtió
una ruta Linux a ruta Windows; el smoke Node evita esa conversión. Ninguno de
esos intentos fallidos se presenta como PASS; se repitieron con éxito.

## Inventario de integración

Se consultaron **individualmente todos los PR abiertos #64–#83**, incluyendo las
ramas de validación #75 y #81. Para cada SHA se ejecutaron `merge-base
--is-ancestor <sha> HEAD` y `rev-list --count HEAD..<sha>`: **20/20 contenidos,
cero commits fuera de la punta consolidada**. Esto no convierte cada PR en
redundante respecto a su propio padre ni autoriza cerrarlo prematuramente.

Antes del commit 1X: `origin/dev...HEAD` = **0 commits solo dev / 93 solo punta**.
La base dev era ancestro, sin divergencia que resolver en ese momento. No se
realizó merge, squash, rebase ni force-push.

Ruta de aterrizaje: un único PR Draft de integración de la punta 1X hacia `dev`,
con revisión focal del diff desde `b81f2800` para lo nuevo. Mantener los PR
originales abiertos hasta aprobación de la consolidación. No añadir otra rama
de validación temporal ni otra pila de PRs comerciales. Los gates normativos,
revisión, aprobación y CI aplicables siguen siendo requisitos previos al merge.

La instrucción de no ejecutar Actions sigue vigente durante la fase local:
publicar con marca explícita `[skip actions]` mantiene los checks **sin ejecutar**,
no aprobados; no cambia workflows, `CI Gate` ni `--require-complete`. Antes de
integrar será obligatorio habilitar una ejecución normal de CI sobre la punta
que se pretenda fusionar. La omisión local no acredita ni elude esos requisitos.

## Estado de producto y próximo paso

`VERSION` permanece `0.4.0-SNAPSHOT`; sin tag, release, despliegue ni retiro de
Spring Boot. Técnicamente existe la vertical de lectura de laboratorio. No hay
sesión comercial, escritura IPC ni esquema candidato activado en producto.

Próximo paso funcional: aceptar explícitamente el bloque Auth/RBAC/Audit/DDL y
el transporte de producto, resolver SYSTEM/instalación/email/fingerprint según
el expediente y conectar sesión real + estas lecturas en el composition root
canónico. Después habilitar una escritura de catálogo CAS/auditable; no repetir
el dominio ni empezar otro harness desconectado.
