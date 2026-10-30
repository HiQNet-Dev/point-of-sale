# Incremento 1Y — de laboratorio Tauri a instalación Local persistente

**Estado: implementado y validado localmente. Sin aprobación normativa, sin merge y
sin release.** No se promovió DDL candidato, no se activó autenticación comercial
ni se retiró Spring Boot.

## 1. Punto de partida verificado

- Rama: `feat/fullstack/rust-desktop-vertical-1x`.
- HEAD local y remoto: `4ab4ae4214cb56a91f86642c1eb5e35bc05d4fbd`.
- PR #84 en Draft, base `dev`, `mergeStateStatus = CLEAN` y árbol limpio.
- `VERSION = 0.4.0-SNAPSHOT` intacta.
- No se dispararon ejecuciones nuevas de Actions durante el trabajo local.

## 2. Qué se implementó

### 2.1 Caso de uso de instalación (`pos-application`)

Nuevo módulo `pos-application::installation`:

| Elemento | Contrato |
| --- | --- |
| `ApprovedSchema` | Versión máxima que este build puede operar |
| `SchemaObservation` | Hecho observado por el adaptador: archivo preexistente, migraciones aplicadas, versión máxima |
| `InstallationReader` | Puerto de lectura del historial |
| `AuthorizedMigrator` | Puerto del migrador aprobado; la aplicación decide *cuándo*, el adaptador *cómo* |
| `prepare_installation` | Clasifica, migra sólo en primera instalación y vuelve a clasificar |
| `InstallationState` | `FirstInstall` / `Reopened` |
| `InstallationError` | Errores acotados: versión inválida, historial incoherente, esquema posterior, lectura no disponible, migración rechazada |

Política: historial vacío ⇒ primera instalación; versión aplicada igual al build ⇒
reapertura; **versión posterior ⇒ se rechaza el arranque**. Nada de esto interpreta
tablas de negocio.

### 2.2 Adaptador y versión aprobada (`pos-storage-sqlite`)

- `installation::SqliteInstallationSchema` implementa ambos puertos sobre el pool
  existente: lee `_sqlx_migrations` y aplica `migrations/local`.
- `approved_schema_version()` deriva la versión del **mismo migrador embebido**, de
  modo que el número no puede desincronizarse del DDL aprobado.

### 2.3 `pos-local`

Ahora informa el ciclo de vida en vez de migrar a ciegas:

```text
installation_ready state=first_install schema_version=1 applied_migrations=1
installation_ready state=reopened    schema_version=1 applied_migrations=1
```

Un esquema posterior se registra como `installation_rejected reason=schema_ahead` y
el proceso termina en fallo sin escribir.

### 2.4 Aplicación de escritorio de producto (`apps/pos-desktop`)

- `tools/desktop-lab` pasa a `apps/pos-desktop` (crate `pos-desktop`); un entrypoint
  pertenece a `apps/`, una herramienta de verificación no.
- Modos explícitos por argumento, sin variables de entorno ni valores por defecto:
  `--synthetic-lab`, `--installation <directorio>`, `--export-bindings`.
- El modo instalación abre `<directorio>/hiqnet-pos.sqlite`, aplica sólo el migrador
  aprobado, clasifica la instalación y expone **un único comando**: 
  `installation_status`.
- Los comandos de laboratorio responden `OPERATION_NOT_AVAILABLE` en modo
  instalación, sin consultar SQL. El laboratorio no expone `installation_status`.
- El cierre hace checkpoint del WAL y cierra el pool; el archivo persiste y nunca se
  borra.
- La ventana decide por sí misma entre pantalla de instalación y catálogo de
  laboratorio consultando el modo real del proceso; el frontend no elige el modo.

## 3. Dependencias físicas y límites

- El modo instalación sólo contiene `0001` (organization/branch). No hay identidad de
  instalación, usuarios, sesiones, catálogo ni ventas.
- **No** se crean usuarios, sesiones, credenciales ni reloj sintético en modo
  producto.
- El identificador de bundle y el directorio por plataforma no se tocaron: dependen
  de `D-INST` (bloque D).
- `proposals/0002`–`0015` siguen fuera del migrador y sólo se ejecutan en `pos-lab`.

## 4. Pruebas ejecutadas

### Workspace headless (`backend-rust`)

```sh
cargo fmt --all -- --check                                     # PASS
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings   # PASS
cargo test  --locked --workspace --all-features                # 290 pruebas, 0 fallos
cargo build --locked --workspace --all-features                # PASS
```

Las 290 incluyen 5 nuevas de política de instalación en `pos-application`, 3 de
adaptador SQLite con reapertura real y 1 de vocabulario de telemetría.

### Workspace gráfico (`apps/pos-desktop`)

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked                                            # 15 pruebas, 0 fallos
```

Cubren: snapshots de bindings, contrato de comandos, guard de capas, argumentos,
modos, aislamiento por organización, revocación/expiración de sesión, eliminación de
permiso, rechazo de esquema posterior, persistencia por reapertura y que el modo
instalación no sirve datos de negocio.

### Frontend

```sh
pnpm lint          # PASS
pnpm exec tsc --noEmit -p tsconfig.json   # PASS
pnpm test          # 211 pruebas, 3 omitidas opcionales
```

### E2E gráfico real (Windows/WebView2)

```json
{"status":"PASS","mode":"synthetic_lab","transport":"tauri_webview2_ipc","consumer":"React","commands":3,"exactUnitPrice":"12.3456","cleanShutdown":true,"productive":false}
{"status":"PASS","mode":"installation","transport":"tauri_webview2_ipc","persistent":true,"schemaVersion":1,"authEnabled":false,"productive":false}
```

El segundo arranca dos veces sobre el mismo directorio y observa
`Primera instalación` → `Instalación reabierta`, con el archivo presente tras el
cierre limpio.

### Docker (Linux, runtime headless)

```sh
node backend-rust/docker/smoke-local.mjs
```

```json
{"status":"PASS","project":"hiqnet-pos-smoke-1793388230685","migrations":"approved only","permissions":"700 10001:10001","durableClosedFile":"b8defd5fe9e832a3ca2b2259f3ee5dd16655503099ecccafcb3533be88cc77b7","firstInstall":true,"reopen":true,"productive":false}
```

El smoke ahora exige evidencia de `first_install`, de `reopened` y un checksum
idéntico del archivo **cerrado** entre reinicios.

## 5. Artefactos

Nuevos:

- `backend-rust/crates/pos-application/src/installation/mod.rs`
- `backend-rust/crates/pos-storage-sqlite/src/installation.rs`
- `backend-rust/apps/pos-desktop/**` (antes `tools/desktop-lab`)
- `backend-rust/apps/pos-desktop/src/{arguments.rs,composition.rs,commands.rs,dto.rs,main.rs}`
- `frontend/src/features/rust-desktop-lab/{DesktopShell.tsx,InstallationPage.tsx}`
- `frontend/scripts/{desktop-e2e.mjs,test-desktop-installation.mjs}`
- `docs/01-architecture/adr/0024-local-persistent-installation.md`
- `docs/05-development/rust-increment-1y-approval-package.md`

Modificados: `pos-local` (dependencia de aplicación, preparación, telemetría),
`connection.rs` (`approved_schema_version`), `pos-storage-sqlite/src/lib.rs`,
`DesktopCatalogPage*` (servicio único), `rust-lab.tsx`, cliente y bindings
generados, scripts de arranque, `tauri.conf.json`, capabilities y permisos, y
documentación de arquitectura, API, tablero y README.

## 6. Bloqueos que persisten

| Bloqueo | Efecto inmediato |
| --- | --- |
| D-AUTH, D-INST, D-AUTH-09, ADR-0022 | Sin login comercial ni sesiones de producto |
| D-CAT / D-CASH | Sin catálogo ni cajas persistentes en producto |
| D-SALES / D-INV | Sin ventas, inventario ni checkout en producto |
| ADR-0021 | Migración de datos de compras sigue bloqueada |
| CI normal | Requiere permiso del titular para reanudar Actions |

El paquete de firma por bloques es
[`rust-increment-1y-approval-package.md`](rust-increment-1y-approval-package.md).
Ninguna casilla está marcada.

## 7. Siguiente paso recomendado

1. Firmar el bloque D (transporte y directorio) para fijar dónde vive la instalación.
2. Firmar el bloque A para activar `0002`–`0007` y sustituir la sesión sintética por
   verificación de credenciales real, en cuanto `D-AUTH-09` tenga evidencia.
3. Con el bloque B firmado, exponer la primera escritura IPC (crear producto) con CAS
   y auditoría sobre la instalación persistente.
