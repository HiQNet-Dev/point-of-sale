# Incremento 2D — contraseña inicial atómica y preparación de identidad Local

**Estado: implementación local verificada; sin activación comercial.**
Base local/remota: `dab25ea7f4de5e98a4011e974233809e3b37999f`, rama
`feat/fullstack/rust-desktop-vertical-1x`, PR #84 Draft → dev. Se preservó el checkout;
no se crearon ramas/worktrees ni se descartaron cambios. Este incremento queda
sin nuevos commits/push; el PR remoto conserva el código publicado de 2C.
No se ejecutan Actions, merge, tags, releases ni cambios de VERSION.

## 1. Bootstrap ya no puede omitir una contraseña utilizable

- `BootstrapRequest` exige `VerifiedInitialCredential`: tipo interno, sin serde y
  sin constructor de campos públicos. El request nunca contiene contraseña en claro.
- Application declara `BootstrapPasswordVerifier`; el adaptador usa las funciones
  criptográficas existentes de `pos-credentials`. No se añade criptografía a Domain
  o Application ni SQL a Application.
- El material inicial debe ser PASSWORD/Argon2id, ACTIVE, versión 1, sin bloqueo ni
  intentos previos. La contraseña real debe verificar el hash, la política y los
  parámetros declarados. Un hash de forma válida no basta.
- La preparación usa sal CSPRNG y prueba de posesión **antes de adquirir la UoW**;
  el consumidor nativo futuro debe ejecutar CPU fuera del hilo UI y bajo admisión.
- `CandidateBootstrapPasswords` se ensambla solo en tests/escenario candidato,
  explícitamente con política LAB. No es una ruta comercial ni un alias de política
  comercial. El runtime instalación no lo llama.
- Application valida ownership respecto al administrador, crea identidad, persiste
  contraseña, asigna grants, escribe auditoría SYSTEM y solo entonces completa el
  registro. La actualización SQL final exige la contraseña ACTIVE y el evento
  SYSTEM correspondiente en la misma conexión.
- Se conserva una sola UoW (`BEGIN IMMEDIATE`), sin commit/pool adicional. El
  llamador debe confirmar solo tras éxito o hacer rollback/abandonar ante error,
  como los contratos transaccionales existentes. No se auto-confirma una excepción.

Pruebas adicionales sobre SQLite candidato:

1. Contraseña correcta/incorrecta contra el material persistido tras reabrir.
2. Debug no contiene contraseña ni hash; lectura de otra organización no encuentra
   la credencial del administrador.
3. Fallos de credencial, rol, grants y auditoría dejan el registro PENDING dentro de
   la UoW; tras rollback no queda organización, usuario, credential, grants o auditoría.
4. Tras retirar el fallo inyectado puede reintentarse sin borrar la instalación.
5. Abandono antes de commit revierte todos los efectos.
6. Credencial de otro usuario o prueba con contraseña distinta se rechazan.
7. Se conservan segundo bootstrap tras reinicio y concurrencia con IDs distintos;
   ambos intentos tienen material válido para su propio administrador.

No se crea una sesión como efecto implícito de bootstrap. Tampoco se acredita la
autoridad comercial solo por estar vacía la instalación.

## 2. Decisión de primer arranque pendiente del titular

[ADR-0025 propuesto](../01-architecture/adr/0025-free-local-first-provisioning.md)
compara A (asistencia nativa con verificación real) y B (token CSPRNG externo,
fuera del renderer), con amenazas, custodia, TTL, consumo, replay, interrupciones,
recuperación, auditoría y tratamiento de instalación existente.

Recomendación: **B condicionado** a aprobar emisor/autoridad, binding, plazo,
recuperación, custodia y estado transaccional. `0007` no almacena hoy el expediente
de token; no se simula con JSON ni se implementa un emisor permisivo. No se exige
un servidor ni se abre HTTP. La elección sigue sin aceptar y no se habilita IPC.

Se reserva como propuesta la versión `free-local-argon2id-v1`, con la candidata
m=19456 KiB/t=2/p=1, sal 16 bytes/salida 32, admisión 4/cola 8/espera 2 s. La prueba
`proposed_commercial_policy_is_not_an_activatable_crypto_policy` comprueba que ese
ID **no** se resuelve en `Argon2Policy`. Umbrales de cuenta/instalación, TTL e
invalidación de sesiones siguen pendientes de decisión/evidencia.

## 3. Identidad, credenciales y sesiones

Se amplió el adaptador existente `SqliteCredentialLab` con selección SQL canónica,
no una segunda implementación de verificación o bloqueo. `canonical()` usa
`local_credential`/`pos_user`, conserva timestamps UTC físicos, traduce
PASSWORD_RESET_REQUIRED y comparte prepare/apply, CAS y reglas de dominio.

Se prueba bloqueo persistente tras cerrar/reabrir, rechazo dentro del enfriamiento,
reset tras verificación correcta al vencer, estado físico de reset y rechazo de
un resultado preparado cuando el usuario fue desactivado. Las mutaciones de la
prueba de bloqueo comparten UoW y auditoría USER con la sesión existente de fixture;
no se presenta esa prueba como un login público de un usuario desconocido.

Sesiones/RBAC, membresías, revocación y expiración existentes se conservan y sus
suites se ejecutan. Faltan login canónico compuesto con emisión de sesión real,
invalidación según cambios, presupuesto global persistente y respuesta/auditoría
prelogin no enumerable. Los resultados internos de credenciales no se exponen
como errores de usuario inexistente por IPC.

**D-AUTH-09 continúa bloqueada.** Se repitió solo la interoperabilidad real con
Java, no una investigación/benchmark general. Falta hardware PyME representativo,
latencias/memoria/saturación con UI y aceptación del titular. Ningún LAB_* se activa
en el modo instalación.

## 4. Bloque D: respaldo verificable preparado

`pos-storage-sqlite::backup` añade:

- `create_backup`: snapshot `VACUUM INTO`, no copia del archivo vivo omitiendo WAL.
- `verify_existing_backup`: reapertura tras reinicio con digest de manifiesto custodiado.
- `restore_to_new_file`: solo a destino nuevo, sin sustituir una instalación.
- Integridad completa SQLite, foreign_key_check, historial SQLx limpio con versiones
  y checksums exactamente aprobados; rechaza futuro/cambio de historial.
- Digest SHA-256 del archivo cerrado; detección de modificación antes/después de copiar.
- Reserva exclusiva `create_new`, incluso frente a destinos vacíos; rechazo de
  auxiliares WAL/SHM/journal previos, sin borrarlos. Archivos nuevos 0600 en Unix.
- E/S de archivos fuera del executor async mediante spawn_blocking; sync de archivo.

Cinco pruebas cubren WAL con frames confirmados, snapshot independiente de escrituras
posteriores, reapertura de manifiesto, recuperación durable, no sobrescritura,
auxiliares huérfanos, corrupción y esquemas futuros/checksums alterados.

**No hay CLI/IPC de backup ni cutover automático.** No se cerraron ACL Windows,
TOCTOU, validación de propietario/almacenamiento compartido, candado cooperativo de
todas las herramientas, fsync del directorio, pérdida de alimentación, directorio
nativo Tauri ni bundle identifier definitivo. La autenticidad/autorización del
manifiesto no se deduce de su digest. Un fallo puede dejar un artefacto parcial
reservado: no se activa ni se borra por fuerza. El entrypoint administrativo debe
validar autoridad/ACL y cerrar/coordinar runtimes antes de una sustitución aprobada.

## 5. Purchasing y frontend

Los 17 handlers preparados y sus bindings se conservan sin registrarlos. Se
refuerzan las pruebas de replay: mismo scope/orden/conjunto de IDs+cantidades;
otra organización con permiso propio, sucursal u orden no acceden al hecho;
partidas diferentes con cantidades iguales también provocan conflicto.
Receipt/ledger/auditoría no se duplican. La clave sigue siendo por organización,
no se cambia silenciosamente a unicidad global. `expectedVersion` es precondición
del primer intento, no impide recuperar un replay idéntico ya confirmado.

La pantalla instalación aclara que esquema FIRST_INSTALL/REOPENED no acredita
administrador o sesión y que identidad no está habilitada; no añade formularios,
credenciales o botones falsos. Dos pruebas comprueban ese cierre. Expirado/revocado,
no autenticado y contexto autorizado tendrán que venir del backend de identidad,
no inferirse desde el frontend. Todavía no hay flujo comercial completo.

## 6. Validación ejecutada

| Control | Resultado |
| --- | --- |
| Rust headless fmt/Clippy estricto/workspace/all-targets/all-features/locked/build | PASS |
| Suite headless completa | **332 PASS**, 0 fallos |
| Workspace desktop Windows fmt/Clippy/test/build | **35 PASS**, 0 fallos |
| Frontend lint/typecheck/build | PASS; warning de chunk >500 kB existente |
| Frontend suite completa con dos workers | **220 PASS**, 3 opt-in skipped |
| Contratos Rust↔TypeScript existentes | **3 PASS** |
| E2E WebView2 existente lab e instalación | Ambos PASS, no comerciales |
| Java BCrypt → Rust y Rust Argon2id → Java | **10 comprobaciones PASS**, dos vectores sintéticos y formatos |
| Linux Docker, módulo de backup actual | **5 PASS** |
| Linux Docker, flujo/bootstrap/credenciales actual | **13 PASS** |
| Docker runtime no-root/smoke de persistencia | PASS: 700 10001:10001, healthcheck, cierre y reapertura |

Linux usa el stage builder existente compilado con las fuentes actuales y ejecuta
las mismas pruebas SQLite, no un backend nuevo. El stage de compilación/pruebas no
se confunde con el runtime no-root ni con E2E gráfico Tauri Linux.

Digest conservado entre arranques del smoke:
`2f13024e64ffadbc478c909a43c865bd06e5832012f9f75d094d17814db0d4b2`.
Evidencia local ignorada por Git: `backend-rust/target/2d-workspace-tests.log` y
`2d-credential-interop.json`. La herramienta de interoperabilidad no publica hashes
ni contraseñas reales. No se ejecutó paridad diferencial completa de Purchasing/POS.

## 7. Gobierno y activación pendiente

- Solo 0001 aprobada. 0002–0016 siguen propuestas, sin cambiar sus bytes/checksums.
- A/B/C sin aprobar; D aceptado como diseño y CC21-03 solo parcialmente.
- [Matriz A 2D](rust-block-a-readiness-2d.md): discrepancias, procedencia y secuencia
  selectiva de promoción; no transforma desarrollo en aceptación del freeze.
- Último estado normativo reportado: 1 PASSED, 28 BLOCKED, 1 NOT_RUN. No se cambia
  ningún estado por una prueba técnica ni se certifica de nuevo el conjunto.
- PR #84 remoto conserva dab25ea7 y permanece Draft. No hay CI de estas modificaciones
  locales, ni ejecuciones nuevas de Actions, ni afirmación de checks verdes.
- Antes de login comercial: decisión de autorización inicial, A/D-AUTH-09,
  composición de credenciales/sesión/auditoría, hardening D y contratos/ACLs explícitos.
- Antes de retirar Java/v0.4.0: B/C/Purchasing autorizados y expuestos, pantallas,
  paridad diferencial, import seguro Local/Central/Control, backup/cutover y reversión,
  instalador, CI normal autorizado en SHA final y Release Audit.
