# ADR-0024 — Instalación Local persistente en la aplicación de escritorio

## Estado y autoridad

**Diseño del bloque D aceptado por el titular el 2026-10-30.** La aceptación es
arquitectónica y de política de almacenamiento: **no** autoriza distribución,
instalador, autoarranque, actualizaciones, release ni cutover.

Este ADR no aprueba D-AUTH, D-INST, D-CAT, D-CASH, D-SALES, D-INV ni ADR-0022,
no promueve DDL candidato y no autoriza distribución, instalador ni release.

Completa a [ADR-0023](0023-tauri-readonly-laboratory.md): el laboratorio de sólo
lectura deja de ser la única superficie de escritorio, pero **el modo producto no
hereda sus fixtures**.

## Contexto

1X demostró `React → Tauri/IPC → Application → SQLite` sobre una base temporal con
sesión y reloj sintéticos. Ese modo no puede ser la aplicación que conserve datos
de una instalación real: crea el esquema en cada arranque y autentica con una
fixture.

La siguiente meta es una instalación que **se abra, conserve datos entre reinicios
y no invente identidad** mientras las decisiones físicas siguen pendientes.

## Decisión

1. El ejecutable de escritorio pasa de `backend-rust/tools/desktop-lab` a
   `backend-rust/apps/pos-desktop` (crate `pos-desktop`) y mantiene workspace y
   lockfile propios para no introducir GTK/WebKit/Tauri en el workspace headless.
   Un entrypoint pertenece a `apps/`; una herramienta de verificación no.
2. El modo se elige **sólo por argumentos**, sin variables de entorno ni valores
   por defecto:
   - `--synthetic-lab`: laboratorio efímero con fixtures sintéticas.
   - `--installation <directorio>`: instalación persistente en
     `<directorio>/hiqnet-pos.sqlite`.
   - `--export-bindings`: contrato TypeScript.
   Una combinación ambigua se rechaza.
3. El modo instalación aplica **exclusivamente** el migrador aprobado embebido
   (`migrations/local`) y clasifica el resultado con un caso de uso de aplicación:
   `pos-application::installation`. Nunca carga `proposals/`.
4. Política de apertura: historial vacío ⇒ primera instalación con migración;
   versión aplicada igual al build ⇒ instalación reabierta; versión aplicada
   posterior al build ⇒ **se rechaza el arranque**. Operar sobre un esquema
   desconocido rompería invariantes antes de que la aplicación pueda detectarlo.
5. El modo instalación **no** crea sesiones, usuarios, credenciales ni reloj
   sintético, y **no** expone operaciones de negocio. Su único comando propio es
   `installation_status`. Los comandos de laboratorio responden
   `OPERATION_NOT_AVAILABLE`, sin consultar SQL.
6. El cierre es ordenado: checkpoint del WAL y cierre del pool. El archivo
   persiste; no se borra ni se reemplaza ningún archivo existente.
7. El puerto de lectura del historial y el migrador autorizado se declaran en
   `pos-application`; el adaptador `SqliteInstallationSchema` los implementa sobre
   el pool existente. `pos-local` reutiliza el mismo caso de uso, de modo que
   contenedor y escritorio comparten política.

## Consecuencias

- Existe una ruta de producto ejecutable que **no** depende de esquema candidato y
  que demuestra durabilidad real entre reinicios, en escritorio y en contenedor.
- La primera instalación y la reapertura dejan evidencia estructurada
  (`installation_ready` con `state` y `schema_version`).
- Sigue sin haber login, RBAC de producto ni escritura comercial: el bloqueo es
  normativo, no técnico. El paquete de decisión está en
  [`rust-increment-1y-approval-package.md`](../../05-development/rust-increment-1y-approval-package.md).
- El identificador de bundle se mantiene sin cambios. Fijar el directorio de datos
  por plataforma e identidad de instalación depende de `D-INST` (bloque D).

## Aceptación del bloque D — 2026-10-30

Registro de la autorización del titular (documento `00B · Control maestro de
revisiones y aprobaciones — HiQNet POS`, 2026-10-30). La aceptación cubre la
política, no su distribución comercial.

| Elemento | Resolución **aceptada** | Implementación |
| --- | --- | --- |
| Transporte principal de Free Local | Tauri v2/IPC; sin servidor HTTP local | Sí (1X/1Y) |
| Base de datos | Una única SQLite por instalación | Sí |
| Directorio de datos | Directorio privado administrado por el sistema operativo | **Pendiente de implementar**: `app_data_dir` por plataforma |
| Identificador de aplicación | Estable y decidido por producto; `com.hiqnet.pos` es candidato, **no fijado** | No (se conserva `local.hiqnet.rustlab` hasta verificar colisiones y distribución) |
| Rutas explícitas | Solo desarrollo, recuperación y herramientas autorizadas | Sí (`--installation`) |
| Rutas desde JavaScript | Prohibidas | Sí (ningún comando las acepta) |
| HTTP local | Prohibido por defecto | Sí (no existe) |
| Separación laboratorio/producto | Modos disjuntos, sin fixtures en producto | Sí |
| Identidades sintéticas en runtime comercial | Prohibidas | Sí (el modo instalación no crea sesiones ni operadores) |

La aceptación **no** fija el directorio ni el identificador: cambiarlos mueve la
ubicación de una instalación existente y exige detección, respaldo y migración
reversible antes de aplicarse.

## Resolución 1Z del bloque D (registro histórico previo a la aceptación)

| Elemento | Resolución propuesta | Implementado en 1Z |
| --- | --- | --- |
| Transporte principal de Free Local | Tauri v2/IPC; sin servidor HTTP local | Sí (ya en 1X/1Y) |
| Base de datos | Una única SQLite por instalación | Sí |
| Directorio de datos | Directorio privado del sistema operativo, elegido por la aplicación | **No**: depende de `D-INST` y del identificador de bundle |
| Identificador de aplicación | Estable y decidido por producto | No (se conserva `local.hiqnet.rustlab`) |
| Rutas explícitas | Solo desarrollo, recuperación y herramientas autorizadas | Sí (`--installation`) |
| Rutas desde JavaScript | Prohibidas | Sí (ningún comando las acepta) |
| HTTP local | Prohibido por defecto | Sí (no existe) |
| Separación laboratorio/producto | Modos disjuntos, sin fixtures en producto | Sí |

> 1Z describió esta tabla como propuesta. El **2026-10-30** el titular aceptó la
> política completa del bloque D; la tabla superior es la referencia vigente.

### Reglas de directorio ya implementadas

`apps/pos-desktop::installation_dir` valida antes de abrir SQLite:

- rechaza un archivo o ruta vacía;
- rechaza un directorio o archivo de base de datos que sea enlace simbólico;
- crea el directorio con permisos `0700` en Unix;
- rechaza un directorio escribible por todos;
- nunca borra ni reemplaza el archivo existente.

Limitación declarada: en Windows los *junctions* no se distinguen de un
directorio con la API estándar; la comprobación cubre enlaces simbólicos. El
bloqueo entre procesos sigue siendo responsabilidad de SQLite (un pool por
proceso, `BEGIN IMMEDIATE` para escrituras críticas).

### Pendiente para cerrar el bloque D

1. Implementado en 2B: `--installation-default` resuelve el directorio de datos
   privado del usuario (`%LOCALAPPDATA%`, `$XDG_DATA_HOME` o `~/.local/share`) y
   lo combina con el directorio estable `com.hiqnet.pos`.
2. Decidir el bundle identifier definitivo (candidato `com.hiqnet.pos`, sin fijar
   hasta verificar colisiones, continuidad de almacenamiento y distribución).
   Cambiarlo mueve la ruta de una instalación existente y exige detección,
   respaldo y migración reversible.
3. Decidir si la instalación admite varias bases (por sucursal o por perfil) o
   exactamente una.
4. Confirmar que la recuperación administrativa usa `--installation <directorio>`
   y no un modo oculto.

## Consolidación técnica 2C — apertura segura (sin nueva autorización)

Este apartado actualiza el estado de implementación; las tablas 1Z/2B anteriores
se conservan como historial. No cambia el alcance de aceptación de 2026-10-30.

- La reapertura ejecuta el migrador aprobado para **validar checksums e historial**,
  no para reaplicar DDL ya ejecutado. También aplica upgrades aprobados pendientes.
  El estado informado sigue describiendo si había historial al abrir.
- Un esquema futuro se rechaza **antes** de invocar el migrador. Un migrador que
  no alcanza la versión aprobada no puede informar disponibilidad.
- Se validan todos los antecesores de la ruta, incluidos junctions/puntos de
  reanálisis en Windows; también SQLite, WAL, SHM, rollback journal y lock file.
- En Unix el directorio final debe ser privado (sin acceso de grupo u otros).
  Si no lo es, se rechaza sin modificar permisos existentes.
- Un candado exclusivo del SO (`.hiqnet-pos.lock`) impide dos runtimes **desktop**
  cooperantes sobre la misma instalación. Se adquiere antes de SQLite y se libera
  después de cerrar el pool; nunca se borra el lock file al cerrar. No sustituye
  `BEGIN IMMEDIATE`, no protege frente a programas que ignoren el candado y no
  se ha añadido al ejecutable headless `pos-local`.

Límites pendientes: validación de ACL Windows (no se presupone que cualquier
ruta explícita sea privada), carreras de sustitución entre validación y apertura
(TOCTOU), directorios compartidos, identidad definitiva del bundle, resolución
nativa Tauri, backup/restauración y actualización de instalaciones. No hay
migración de rutas automática ni activación de esquemas candidatos.

Evidencia y alcance: [avance 2C](../../05-development/rust-increment-2c.md).

## Preparación de mantenimiento 2D — sin activación administrativa

`pos-storage-sqlite::backup` añade `VACUUM INTO`, verificación de integridad/FKs e
historial/checksums, digest del archivo cerrado y restauración **a archivo nuevo**.
Rechaza destinos existentes (incluso vacíos) y auxiliares WAL/SHM/journal previos;
no elimina archivos ni activa un destino. Un snapshot puede reabrirse con el digest
de un manifiesto custodiado antes de restaurarlo. Ese digest no acredita autoridad.

Son componentes de infraestructura probados, **no comandos de producto**. El
entrypoint administrativo futuro debe aplicar ACL/directorio privado, autorización,
protocolo de candado compartido y cierre controlado para cutover. No se copia el
archivo abierto de una instalación como si incluyera automáticamente su WAL.
Siguen pendientes ACL Windows efectiva, TOCTOU, coordinación con pos-local,
identificador/ruta nativa, custodia del manifiesto, fsync de metadatos del directorio
y simulación de fallos de alimentación. No se cambia el alcance de D aceptado.

## Alternativas descartadas

- **Reutilizar el laboratorio como producto**: mezclaría fixtures con datos
  reales y afirmaría identidad inexistente.
- **Usar `app_data_dir` con el identificador de laboratorio**: situaría datos de
  producto bajo `local.hiqnet.rustlab`; 2B resuelve la ruta con el directorio
  estable del producto y deja el bundle identifier como decisión separada.
- **Abrir un servidor HTTP para probar**: contraviene ADR-0020 y ampliaría la
  superficie de red sin decisión.
- **Ignorar el esquema más nuevo y operar igualmente**: convierte una
  incompatibilidad en corrupción silenciosa.
