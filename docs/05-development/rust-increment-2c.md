# Incremento 2C — consolidación, bootstrap canónico y Purchasing preparado

**Cierre técnico del alcance mínimo de la continuación; activación comercial pendiente.**
No se declara terminado el reemplazo Java/Rust ni autorizado v0.4.0.

## 1. Base, preservación y publicación

Base local/remota verificada: `0e7afa3251a229a6b6882d9a60c3ef22c868c3e9`, rama
`feat/fullstack/rust-desktop-vertical-1x`, PR #84 Draft hacia `dev`. Se preservaron
los nueve archivos locales de la primera entrega de 2C; no hubo reset, clean,
force-push, cambios de rama ni worktrees nuevos.

Commits enfocados de esta continuación (el informe final se añade después):

| Commit | Responsabilidad |
| --- | --- |
| `cafc8e47` | Consolidar historial/checksums, directorios seguros y candado desktop |
| `ac263072` | Bootstrap en Application, unicidad de instalación y grants del administrador |
| `6cc4a944` | Scope/payload del reintento de recepción |
| `f0572112` | Exclusión entre procesos y recuperación tras terminación forzada |
| `b05a501d` | 17 contratos/handlers Purchasing preparados, bindings y servicio/Query TypeScript |

Todos llevan `[skip actions]`. Los dos workflows inspeccionados (`ci.yml` y
`rust-contracts.yml`) usan eventos push/pull_request o dispatch manual, compatibles
con la omisión nativa de GitHub. No se modificó ningún workflow. La publicación
se realiza en la misma rama; no se solicita merge, tag, release ni ejecución de
Actions. Los commits omitidos no constituyen evidencia CI: antes de integrar se
requiere autorización para CI normal sobre el SHA definitivo.

## 2. Integridad y directorios consolidados

La reapertura ya no devuelve éxito sin validar SQLx. Se reprodujo primero una
aceptación incorrecta de un checksum alterado; ahora se rechaza. El caso de uso
observa, rechaza esquema futuro antes de migrar, ejecuta el migrador aprobado en
cada apertura y comprueba que alcance la versión esperada. No reaplica DDL ya
confirmado ni carga propuestas. Se prueban historial fallido, checksum alterado,
upgrade aprobado pendiente y errores de arranque sin dejar el candado adquirido.

La validación cubre antecesores, symlinks, junctions/reparse points Windows y los
archivos principal/WAL/SHM/journal/lock. En Unix exige directorio final privado y
no cambia permisos existentes silenciosamente. La verificación de historial no
certifica que un administrador no haya alterado manualmente tablas de negocio.

El candado `.hiqnet-pos.lock` se adquiere antes de SQLite y se mantiene hasta
cerrar el pool. Nunca se elimina al cerrar. Se prueba exclusión en otro proceso,
terminación forzada de ese proceso y adquisición posterior sin borrar el archivo.
Es **cooperativo y desktop**, no exclusión global: `pos-local` y herramientas que
no participen pueden ignorarlo. No sustituye las transacciones de SQLite.

### ACL Windows y TOCTOU: límites constatados

Se inspeccionó el ACL real del directorio Local de datos del perfil con `Get-Acl`.
Además de propietario/SYSTEM/Administradores apareció una ACE de otro principal
con lectura/ejecución. No se modificó el ACL ni se publicó su SID. Por tanto, no
se presupone que todo directorio del perfil o de recuperación sea privado.

Pendientes: verificación ACL nativa y efectiva, directorios compartidos, enlaces
sustituidos entre validación/apertura (TOCTOU), ownership/hardlinks, continuidad
ante cambio de bundle identifier, APIs nativas Tauri para ruta, backup/restore y
actualizaciones. Las comprobaciones actuales no son aperturas atómicas no-follow.
No se movieron bases existentes ni se fijó el identificador comercial definitivo.

Respaldo/recuperación deben operar con desktop cerrado y conexiones drenadas;
no borrar el lock file para forzar una apertura. Antes de operar concurrentemente
con desktop, una herramienta autorizada deberá participar en el mismo protocolo
y coordinar SQLite. No se añade todavía un modo de restauración o respaldo.

## 3. Bootstrap canónico: código real, todavía candidato

Se extrajo de infraestructura la coordinación existente, sin otro servicio paralelo:

- `pos-application::bootstrap::{BootstrapRequest, BootstrapTransaction,
  requires_provisioning, provision_initial_installation}`.
- `SqliteTransaction` implementa el puerto sobre la UoW abierta. No abre otro pool
  ni confirma por su cuenta. El llamador conserva `BEGIN IMMEDIATE`, commit/rollback.
- Application construye Organization y `bootstrap.completed` con atribución SYSTEM
  y sin sesión. Exige `BootstrapProvisioning`, emitida solo por `authorize_bootstrap`.
- Crea instalación, administrador, rol y vínculos; concede al administrador los
  **59 permisos del catálogo** en la misma UoW, reutilizando la política que el
  escenario existente aplicaba desde fuera. Se retiró esa duplicación del escenario.
- La detección no depende de IDs del cliente: organización/instalación/usuario o
  registro inicial existentes/parciales impiden comenzar de nuevo. Dos intentos
  concurrentes con IDs distintos solo pueden confirmar uno.
- Auditoría fallida revierte organización, instalación, administrador, rol, grants
  y bootstrap. Se comprueba persistencia y rechazo de otro bootstrap tras reabrir.
- Se conserva el import anterior desde storage mediante reexportación, no una
  segunda implementación. El runtime comercial no invoca la ruta candidata.

### Decisión mínima pendiente para Free Local

El contrato existente exige **secreto de despliegue verificado** antes de emitir
la capacidad. Detectar «no hay administrador» no autoriza a cualquiera a tomar la
instalación. Falta resolver cómo se provisiona, custodia y entrega ese secreto en
el primer arranque Free Local sin Spring Boot, y quién puede recuperar el control.
No se sustituye por un botón allow-all, ruta JS ni usuario fijo.

Este caso todavía **no crea/verifica una credencial inicial ni emite una sesión**.
Esas capacidades existen separadas como candidatas y deben componerse con política
aprobada y una UoW coherente. No se declara login/primer administrador comerciales
operativos por haber completado el provisioning de identidad y sus grants.

## 4. Auth/RBAC y D-AUTH-09

Se reutilizan `LocalPrincipalService`, las lecturas canónicas SQLite, revocación,
expiración, `LockoutPolicy` y las transiciones de credenciales existentes. Las
pruebas comprueban permisos retirados y revocación efectivos en la siguiente
llamada, CAS y rollback. No se reemplazó esa lógica ni se repitieron benchmarks.

La admisión acotada y la interoperabilidad BCrypt/Argon2id de 2B se conservan.
**D-AUTH-09 sigue bloqueada**: falta hardware objetivo y aprobación; no se activan
`LAB_V0`/`LAB_V1` como políticas comerciales. Siguen pendientes el contrato final
de intentos/enfriamiento, límite por instalación persistente, comportamiento de
cuenta inexistente y composición de login/rehash/sesión con protección de enumeración.
No se anuncia terminada una política comercial solo por existir primitivas probadas.

## 5. Purchasing: 17 handlers y frontera frontend preparada

Los 17 casos de uso se reutilizan. Se añaden DTOs independientes, handlers Tauri,
registro preparado, bindings Specta, servicio TypeScript inyectable y opciones
TanStack Query. **No se registran en el dispatcher activo, no se añaden ACLs y el
estado no tiene constructor productivo.** Solo pruebas pueden ensamblarlo ahora.
El singleton frontend falla cerrado sin invocar IPC; no se conecta ninguna pantalla
comercial a fixtures ni se cambia el backend Java de las pantallas existentes.

Cada llamada preparada reconstruye el principal desde sesión backend y SQLite;
no acepta actor, organización, sesión ni ruta desde JS. El reloj/generador del
contexto de prueba se inyectan solo para reutilizar las fixtures existentes; no se
crea otra identidad sintética de producto.

Las versiones y decimales son texto; costos desconocidos siguen `null`, no cero.
Los patches usan `keep`/`clear`/`set` explícitos para no perder la distinción al
deserializar. Los errores se traducen sin SQL/secretos; no hay reintentos automáticos
de mutaciones/CAS. El servicio no repite cálculos empresariales.

La prueba de bindings compara lo regenerado, comprueba los 17 nombres en la guía
y que el registro activo no los exponga. Se amplía únicamente la excepción ESLint
ya existente para el runtime generado por Specta al nuevo archivo generado; no se
modifica ese runtime a mano ni se desactiva lint en código de aplicación.

### Defecto de seguridad encontrado en la integración

El reintento de recepción devolvía el hecho encontrado por clave de organización
antes de comprobar sucursal, orden o cantidades. La regresión de otra sucursal
falló antes del arreglo (devolvía éxito). Ahora Application exige misma sucursal,
misma orden y mismo conjunto de cantidades exactas; rechaza cambio de scope/payload
sin nuevo movimiento/recepción/auditoría y mantiene el replay legítimo. Hay pruebas
headless SQLite y del handler preparado.

Correspondencia completa Java → Rust → comando, permisos, requests/responses,
patches, paginación, errores y límites:
[Purchasing IPC preparado](../03-api/rust-purchasing-ipc.md).

## 6. Evidencia ejecutada en la continuación

| Control | Resultado |
| --- | --- |
| Headless fmt, Clippy `-D warnings`, workspace/all-targets/all-features/locked, build | PASS |
| Suite headless completa, arquitectura y SQLite incluidas | **318 PASS**, 0 fallos |
| Workspace desktop Windows: fmt, Clippy estricto, test y build | **35 PASS**, 0 fallos |
| Directorios/candado Linux Docker sin red | **11 PASS**; incluye un helper de proceso, no 11 escenarios comerciales |
| Frontend lint/typecheck | PASS |
| Suite frontend completa, `vitest run --maxWorkers=2` | **218 PASS**, 3 opt-in skipped |
| Contratos Rust↔TS existentes | **3 PASS** |
| Frontend build | PASS; warning de chunk >500 kB existente |
| E2E React/WebView2/IPC lab e instalación persistente | Ambos PASS, `productive: false`, instalación sin auth |
| Docker build/runtime/smoke | PASS: no-root, `700 10001:10001`, healthcheck, cierre, reapertura |
| Escenario POS existente dentro de la imagen Linux actual, sin red/read-only | PASS: venta COMPLETED, turno CLOSED, inventario exacto; candidato/sintético |

Digest del archivo cerrado conservado entre los dos arranques del smoke final:
`a0be5724b371c78a1e499f69ea66e8aabbe1087874ad33442f0754df744a01ad`.
Log local ignorado por Git: `backend-rust/target/2c-workspace-tests.log` (318 PASS).

Se agotaron límites de tiempo de las primeras ejecuciones paralelas headless/Docker;
la suite frontend por defecto también tuvo fallos de tiempo y una expectativa de
navegación bajo esa carga. No se modificaron las pruebas ni sus timeouts. Las
repeticiones completas secuenciales y frontend con dos workers pasaron. Se corrigió
además el setup del test nuevo: no debe retornar el mock desde `beforeEach`, pues
Vitest lo interpretaba como teardown y volvía a ejecutar una función rechazada.

Los 17 handlers se prueban por llamada Rust con estado Tauri y SQLite candidato;
**no es dispatcher IPC real de Purchasing ni E2E comercial**. TypeScript prueba el
cliente generado con `invoke` simulado y QueryClient. Los E2E gráficos solo validan
los comandos activos previos. Linux compila el módulo de directorios actual y sus
pruebas de proceso; no se declara compilación/E2E gráfico completo Tauri en Linux.

## 7. Estado normativo e integración

| Bloque | Estado |
| --- | --- |
| A | No aprobado; provisioning Application avanzado, pero credencial/login, secreto Local y reconciliación física pendientes |
| B | No aprobado; depende de identidad comercial y promoción de `0008`–`0010` |
| C | No aprobado; depende de A/B y decisiones físicas para `0011`–`0015` |
| D | Diseño aceptado 2026-10-30; apertura/candado avanzados, ACL/TOCTOU/identificador/backup pendientes |
| CC21-03 | Aprobación parcial conservada; no autoriza costo efectivo ni ADR-0021 completo |
| D-AUTH-09 | Bloqueada, sin nueva medición de hardware objetivo ni aceptación |

Solo `0001` autorizada en `migrations/local`; `0002`–`0016` siguen propuestas.
Flyway, SQLx histórico, VERSION y gates intactos. No hay E2E productivo completo,
importador autorizado ni paridad diferencial comercial completa. Último estado
normativo reportado: **1 PASSED, 28 BLOCKED, 1 NOT_RUN**, sin reclasificación nueva
ni certificación del conjunto en esta continuación.

Antes de `dev`: revisión, aprobaciones aplicables y CI normal autorizado sobre el
SHA definitivo, sin omisión. Antes de retirar Java/v0.4.0: identidad auténtica,
esquema productivo, pantallas/contratos activados, paridad diferencial, importación
con política Local/Central/Control, backup/restore, instalador, reversión y Release
Audit. Publicar este PR Draft no autoriza ninguno de esos pasos.

La revisión de los 33 archivos del diff no encontró bases SQLite, WAL/SHM,
`.env`, directorios target/dist/node_modules ni resultados temporales incluidos.
Una comprobación de marcadores de claves privadas/AWS no encontró coincidencias;
no equivale a un análisis exhaustivo de secretos. Gitleaks no estaba instalado.

No había herramientas de subagentes ni Context7 disponibles. Se usaron las skills
instaladas aplicables y las APIs/patrones ya presentes, sin nuevas dependencias.
La revisión fue propia; no se atribuye una auditoría independiente inexistente.
