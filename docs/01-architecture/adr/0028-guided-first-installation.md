# ADR-0028 — Primera instalación guiada y custodia del token de provisioning

**ACEPTADA para la release técnica v0.4.0.** Define cómo una instalación vacía se
convierte en una instalación operable sin que el token de provisioning cruce la
frontera del renderer.

## Contexto

El emisor nativo (`pos-local --provision-token-issue <directorio>`) crea una
autorización `PENDING` y entrega el token en un archivo privado del directorio de
instalación. El caso de uso de bootstrap ya consumía esa autorización de forma
atómica en una única UoW, pero no existía camino de producto para invocarlo: el
escritorio solo sabía abrir la base y, desde ADR-0027, autenticar.

## Decisión

1. **Lectura nativa.** `pos-storage-sqlite::native_authorization` lee
   `provisioning-authorization.txt` con las mismas garantías que el emisor: sin
   enlaces, owner efectivo, sin bits de grupo/otros en directorio ni archivo.
   Windows responde `UnsupportedPlatform` (fail-closed).
2. **Clasificación sin consumo.** `bootstrap_status` compara el token leído con
   el digest persistido y reporta `READY`, `AWAITING_AUTHORIZATION`,
   `AUTHORIZATION_EXPIRED`, `AUTHORIZATION_REVOKED`, `AUTHORIZATION_CONSUMED`,
   `INVALID_AUTHORIZATION`, `INSECURE_PERMISSIONS` o `UNSUPPORTED_PLATFORM`.
3. **Capacidad derivada del token.** `authorize_bootstrap` recibe el token leído
   del archivo privado, comparado en tiempo constante. No existe un bypass por
   “instalación vacía”: sin derecho vivo, `bootstrap_install` responde
   `Unauthorized`.
4. **Un solo commit.** Organización, `installation_identity`, administrador,
   credencial Argon2id, rol, 59 grants, `bootstrap.completed` SYSTEM y el consumo
   de la autorización comparten la misma `BEGIN IMMEDIATE`. Un fallo revierte
   todo; la autorización no queda consumida.
5. **Token fuera de IPC.** Ningún DTO, comando, binding, log, URL o almacenamiento
   del navegador transporta el token ni su digest. El archivo se elimina tras el
   commit (best effort).
6. **Contraseña en Rust.** El wizard envía la contraseña inicial por IPC; el
   hashing y la persistencia ocurren en Rust. Se exige un mínimo de 12 caracteres;
   Argon2id usa la política `lab-argon2-v1` mientras D-AUTH-09 no esté ratificada.
7. **Estados explícitos.** El asistente nunca convierte un fallo en “instalación
   nueva”: cada estado tiene su remedio y se vuelve a consultar a Rust.
8. **Datos mínimos.** Nombre del negocio, administrador, correo, contraseña y
   moneda. `timezone` se fija en `UTC` y `locale` en `es-MX`; su selección es un
   slice posterior.

## Consecuencias

- Una persona puede configurar su negocio, crear el administrador y llegar al
  login sin tocar la base ni el token.
- La autorización sigue siendo de un solo uso y con ventana temporal.
- Windows no puede completar el asistente hasta que exista el componente nativo
  de DACL/owner; el comportamiento previo (fail-closed) se conserva intacto.
- El operador debe colocar el archivo con el emisor nativo; no hay copia manual
  del token desde la interfaz.

## Evidencia

`pos-storage-sqlite::native_authorization_tests` (Linux, SQLite productiva):
permisos inseguros de directorio y archivo, archivo ausente, enlace simbólico,
digest que no coincide, caducidad, derecho consumido, y el recorrido completo
archivo → bootstrap → login → catálogo → reapertura.

`pos-desktop` (Windows): `bootstrap_status` responde `UNSUPPORTED_PLATFORM`, el
comando falla cerrado y ningún intento sin autorización crea `pos_user`.
