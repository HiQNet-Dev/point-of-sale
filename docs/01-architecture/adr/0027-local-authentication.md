# ADR-0027 — Autenticación Local con sesión custodiada en Rust

**ACEPTADA para la release técnica v0.4.0.** Define el camino de autenticación de
HiQNet POS Free Local sobre SQLite y Tauri, y el reparto de responsabilidades
entre dominio, aplicación, persistencia y shell nativo.

No aprueba por sí sola la política comercial de Argon2id (D-AUTH-09), que se
registra en `docs/05-development/rust-auth-benchmark-d-auth-09.md`.

## Contexto

Hasta este incremento la instalación persistente sabía abrir su esquema pero no
autenticar: no existía caso de uso de login, ni sesión comercial, ni IPC de
identidad. El único camino era el adaptador de laboratorio (`*_lab`), que no es
almacenamiento productivo.

El esquema `0003`–`0006` ya define `pos_user`, `local_credential`,
`local_session`, `role`, `permission`, `role_permission`,
`user_organization_role`, `branch_membership` y `audit_event`. `pos-core` ya
modela credencial, bloqueo, sesión, principal verificado y auditoría.

## Decisión

1. **Caso de uso en `pos-application::identity`.** Orquesta login, identidad
   actual, selección de sucursal, logout y logout-all. No conoce SQL ni
   criptografía: depende de los puertos `LoginDirectory`, `IdentityUnitOfWork`,
   `IdentityTransaction`, `PasswordGate` y `SessionSecrets`.
2. **Criptografía fuera del hilo de la transacción.** La verificación Argon2id
   corre en `spawn_blocking`; la transacción de escritura solo se abre después,
   con relectura y CAS sobre `credential_version`.
3. **Un único error de autenticación.** Usuario inexistente, contraseña errónea,
   cuenta inactiva, credencial bloqueada o reset pendiente devuelven
   `InvalidCredentials`. Cuando no hay usuario se ejecuta un coste criptográfico
   equivalente para no ofrecer una señal por latencia.
4. **El handle no llega al renderer.** El proceso Rust custodia el handle de
   sesión en memoria (`tokio::sync::Mutex<Option<String>>`). `auth_login` devuelve
   solo identidad y permisos; `auth_me`, `auth_logout`, `auth_logout_all`,
   `auth_select_branch` y `auth_list_branches` usan el handle nativo. No se
   escribe en `localStorage`, `sessionStorage`, archivos ni SQLite.
5. **Solo el digest se persiste.** `local_session.session_hash` guarda SHA-256 del
   handle; un volcado de la base no permite reutilizar la sesión.
6. **Reinicio = nueva autenticación.** Al cerrar el proceso el handle desaparece.
   Tras reabrir, la persona vuelve a autenticarse; el usuario, sus permisos y su
   historial permanecen en SQLite. No se implementa custodia de secretos del SO en
   este incremento.
7. **Autorización DB-authoritative.** Cada operación revalida sesión, usuario,
   organización, estado y permisos en SQLite (`LocalPrincipalService` sobre
   `TableSet::Canonical`). El renderer no aporta actor, organización ni permisos.
8. **Sucursal.** `auth_select_branch` exige que la sucursal exista en la
   organización del principal, esté `ACTIVE` y sea descubrible con `branch.read`.
   Una sucursal de otra organización responde `BranchNotFound`, sin revelar su
   existencia.
9. **Auditoría.** Login correcto: `auth.login` con la sesión recién creada.
   Rechazo con usuario resuelto: `auth.login_failed` (USER sin sesión, único
   constructor de ese tipo). Logout: `auth.logout`. Logout global:
   `auth.logout_all`. Todos comparten la UoW de la mutación.
10. **Modos.** En `--synthetic-lab` los comandos `auth_*` responden
    `OperationNotAvailable`; en la instalación persistente los `lab_*` siguen
    rechazados.

## Consecuencias

- El administrador creado por provisioning puede autenticarse, obtener sus 59
  permisos, seleccionar sucursal, cerrar sesión y volver a autenticarse tras
  reiniciar.
- Si el renderer se compromete no obtiene el handle: solo puede invocar comandos
  mientras el proceso autenticado siga vivo.
- La sesión no sobrevive al cierre de la aplicación; es una decisión explícita,
  no un olvido. Custodiarla en el SO requeriría un componente de plataforma
  aprobado y probado.
- El bloqueo es 5 intentos / 15 minutos, equivalente al backend Java.
- `rehash_required` de material BCrypt legado se detecta pero no se aplica en
  este incremento; queda como deuda explícita.

## Evidencia

`pos-storage-sqlite::session_tests` (10 pruebas sobre SQLite real y migrador
productivo): login, identidad, permisos, sucursal, logout, logout-all,
reapertura, re-login, contraseña errónea, usuario inexistente, cuenta inactiva,
bloqueo, sesión vencida, sesión revocada, replay, tenant ajeno, permiso retirado,
fallo de auditoría con rollback, intentos concurrentes y TTL fuera de política.

`pos-desktop` pruebas 36/36: los comandos `auth_*` están registrados, los
bindings generados coinciden y el modo laboratorio los rechaza.
