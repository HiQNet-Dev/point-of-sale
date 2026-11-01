# ADR-0029 — Custodia nativa de archivos privados en Windows

**ACEPTADA para la release técnica v0.4.0.** Define la frontera de plataforma que
permite a HiQNet POS Free Local emitir, custodiar y consumir la autorización de
provisioning en Windows sin depender de permisos de archivo sin verificar.

## Contexto

ADR-0025 y ADR-0028 dejaron el provisioning en Unix con una política verificable:
directorio `0700`, archivo `0600`, propietario igual al usuario efectivo y sin
enlaces. En Windows esa verificación no existía y todo el camino respondía
`UnsupportedPlatform` / `OperationNotAvailable`: el asistente de primera
instalación no podía completarse y el criterio de producto «instalar, entrar y
trabajar en Windows» no se cumplía.

Windows no tiene bits POSIX: la autorización efectiva vive en el propietario del
objeto y en su DACL. Comprobarlo mal es peor que no comprobarlo, porque produce
una falsa sensación de custodia.

## Decisión

1. **Un crate de frontera, pequeño y auditable.** `pos-windows-security`
   concentra la única superficie con `unsafe` del producto. Declara
   `unsafe_code = "allow"` en su `Cargo.toml` y `pos-core`/`pos-application`
   conservan `unsafe_code = "forbid"`. El FFI se limita a `win/native.rs`
   (`windows-sys` 0.61, mantenido por Microsoft); la política se escribe en Rust
   seguro.
2. **Política de acceso explícita.** Un objeto es privado si su propietario es el
   usuario efectivo o el grupo `Administrators` —un proceso elevado crea objetos
   a nombre de ese grupo, que ya puede tomar posesión de cualquier archivo, así
   que aceptarlo no amplía el acceso— y ninguna entrada de *concesión* de su DACL
   apunta a otra identidad. Solo se admiten el propio usuario, `SYSTEM`
   (`S-1-5-18`) y `Administrators` (`S-1-5-32-544`), más `CREATOR OWNER` y
   `OWNER RIGHTS`, que Windows resuelve al propietario. Las entradas de denegación
   y las etiquetas de integridad se ignoran: no amplían el acceso.
3. **DACL privada y heredable.** Directorios y archivos se crean con el
   propietario explícito y
   `D:P(A;OICI;FA;;;<usuario>)(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)`: protegida frente
   a la herencia del padre y heredable por los hijos, de modo que el archivo
   SQLite, `-wal`, `-shm`, `-journal` y el candado nacen con la misma DACL. El
   propietario se declara en el descriptor para que el objeto quede a nombre del
   usuario efectivo aunque el proceso se ejecute elevado.
4. **Creación atómica.** La DACL se aplica en la misma llamada que crea el objeto
   (`CreateDirectoryW` / `CreateFileW` con `SECURITY_ATTRIBUTES`), sin ventana en
   la que un archivo con el token exista con permisos heredados. Los archivos se
   crean con `CREATE_NEW` (exclusividad) y los nombres se validan como componente
   simple de ruta antes de tocar el sistema de archivos.
5. **Verificación sobre el handle, no sobre la ruta.** Se abre con
   `FILE_FLAG_OPEN_REPARSE_POINT` para detectar enlaces y junctions en lugar de
   seguirlos, se comprueban los atributos y el descriptor del handle abierto, se
   leen los bytes de ese mismo handle y se exige que su ruta final cuelgue del
   directorio validado. Sustituir la ruta no cambia lo que se lee.
6. **Nunca se corrige en silencio.** Una instalación preexistente con permisos
   amplios se rechaza; no se reescribe su DACL. La política se aplica solo al
   crear y se verifica siempre al reabrir.
7. **Fallo cerrado fuera de Unix y Windows.** Las demás plataformas conservan
   `UnsupportedPlatform`.

Integración: `pos-local` (emisor `--provision-token-issue|status|revoke`),
`pos-storage-sqlite::native_authorization` (lectura del archivo de autorización)
y `pos-desktop::installation_dir` (directorio de instalación, archivo SQLite,
auxiliares y candado).

## Consecuencias

- Windows deja de estar bloqueado: emite la autorización, completa el bootstrap,
  inicia sesión y usa el catálogo sobre SQLite con DACL verificada.
- Un directorio heredado del perfil que conceda acceso a grupos del sistema se
  rechaza. Es deliberado: el producto crea su propio directorio con DACL
  protegida, y una ruta ajena con permisos amplios no es aceptable para un
  archivo que contiene un token.
- Los tests negativos no dependen de privilegios: comparan el descriptor real, no
  el resultado de un intento de acceso. Los permisos de administrador pueden
  tomar posesión de un objeto, así que la prueba estructural es la evidencia
  válida.
- Queda fuera del alcance de esta decisión el cifrado en reposo y la protección
  frente a código que se ejecute con la identidad del propio usuario o con
  control administrativo del equipo.

## Evidencia

En Windows nativo (`cargo test -p pos-windows-security`, 27 pruebas; el resto de
la suite del workspace y del escritorio ejecutada en el mismo entorno):

- directorio creado con propietario efectivo, DACL protegida y solo las tres
  identidades admitidas, también si el proceso se ejecuta elevado;
- rechazo de DACL compartida en directorio y archivo, en lectura y en escritura;
- rechazo de propietario distinto del usuario y de `Administrators`;
- rechazo de junction, enlace simbólico, punto de reanálisis y archivo fuera del
  directorio validado;
- creación exclusiva con un único ganador entre cuatro procesos concurrentes;
- rechazo de nombres que escapan del directorio;
- recorrido completo emisor → archivo privado → bootstrap → login → catálogo →
  reapertura, en `pos-local` y `pos-storage-sqlite`, y de primera instalación por
  IPC en `pos-desktop`.

El parser SDDL (`pos-windows-security::sddl`) se prueba además en Linux, donde no
hay FFI, para que la política no dependa del sistema anfitrión.
