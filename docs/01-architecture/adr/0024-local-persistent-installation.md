# ADR-0024 — Instalación Local persistente en la aplicación de escritorio

## Estado y autoridad

**Implementación técnica solicitada en 1Y; activación comercial NO aprobada.**
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

## Alternativas descartadas

- **Reutilizar el laboratorio como producto**: mezclaría fixtures con datos
  reales y afirmaría identidad inexistente.
- **Resolver el directorio con `app_data_dir`**: la ruta depende del
  identificador de bundle, que es una decisión de identidad de producto; se
  pospone hasta el bloque D.
- **Abrir un servidor HTTP para probar**: contraviene ADR-0020 y ampliaría la
  superficie de red sin decisión.
- **Ignorar el esquema más nuevo y operar igualmente**: convierte una
  incompatibilidad en corrupción silenciosa.
