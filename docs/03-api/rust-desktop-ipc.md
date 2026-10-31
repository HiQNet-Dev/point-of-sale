# Rust desktop — vertical IPC real (1X) e instalación persistente (1Y)

**Sin operaciones comerciales.** El laboratorio usa fixtures sintéticas; el modo
instalación sólo informa su estado. Java HTTP/OpenAPI y las pantallas existentes
siguen operativos. No hay servidor HTTP Rust. Ver
[ADR-0023](../01-architecture/adr/0023-tauri-readonly-laboratory.md) y
[ADR-0024](../01-architecture/adr/0024-local-persistent-installation.md).

## Modos de arranque

El ejecutable `backend-rust/apps/pos-desktop` exige un modo explícito; no hay
configuración productiva por defecto ni lectura de variables de entorno.

| Modo | Argumento | Base de datos | Identidad | Comandos |
| --- | --- | --- | --- | --- |
| Laboratorio | `--synthetic-lab` | Temporal, nueva en cada arranque | Fixture sintética fija | `lab_*` |
| Instalación | `--installation <directorio>` | `<directorio>/hiqnet-pos.sqlite`, persistente | **Ninguna** | `installation_status` |

El modo instalación aplica únicamente el migrador aprobado
(`migrations/local`). Historial vacío ⇒ primera instalación; versión igual al
build ⇒ reapertura; versión posterior ⇒ el arranque se rechaza. En cada apertura
se valida el historial/checksums con SQLx; si faltan migraciones **aprobadas**, se
aplican antes de informar disponibilidad. El único cargador de DDL es
`migrations/local` (`0001`–`0017`, ADR-0026); nunca crea usuarios, sesiones ni
credenciales.

Desde la consolidación 2C, una segunda instancia desktop sobre la misma
instalación se rechaza antes de abrir SQLite. El candado `.hiqnet-pos.lock` se
mantiene hasta cerrar el pool y no se borra: **no eliminarlo para intentar abrir
una segunda instancia**. También se rechazan redirecciones en directorios
antecesores y en archivos SQLite auxiliares. En Unix, un directorio existente
con acceso de grupo u otros usuarios se rechaza sin cambiar sus permisos; una
ruta explícita de recuperación requiere revisión de permisos/ACL por el
administrador. Estos rechazos son fallos de arranque, no nuevos errores IPC.

## Levantar la aplicación para probarla

En Windows con Rust 1.99.1, herramientas MSVC y WebView2 instalados:

```sh
cd frontend
pnpm install --frozen-lockfile
pnpm desktop:lab                      # laboratorio efímero
pnpm desktop:install -- C:/datos/hiqnet-pos   # instalación persistente
```

`desktop:lab` construye una entrada React separada y abre una ventana Tauri. No
necesita Spring Boot, PostgreSQL, Docker ni Vite escuchando. Muestra sucursales,
categorías y productos desde **SQLite real**, con el precio exacto `12.3456`.

`desktop:install` reutiliza la misma ventana: al abrir por primera vez muestra
`Primera instalación`, versión de esquema y migraciones aplicadas; al cerrar y
volver a abrir muestra `Instalación reabierta` sobre el mismo archivo. Ninguno de
los dos modos acepta rutas, organización, actor ni sesión desde JavaScript.

## Camino ejecutado

```text
DesktopShell (React + TanStack Query)
 → desktopService
 → desktop.generated.ts (Tauri Specta)
 → IPC Tauri / WebView2
 → commands.rs (solo adaptación)
 → LocalState: modo laboratorio o modo instalación
 → Application y puertos
 → adaptadores SQLite existentes
 → DTO tipado → React
```

La fixture de `pos-lab` se comparte como biblioteca sólo en el modo laboratorio.
`pos-local` y el modo instalación conservan exclusivamente el migrador aprobado,
sin cargar `proposals/`. El caso de uso `pos-application::installation` es común a
contenedor y escritorio.

## Contratos registrados

Fuente verificable: `apps/pos-desktop/src/commands.rs` + `dto.rs`, relativa a
`backend-rust/`. El mismo registro configura dispatcher y generación de bindings.

| Comando real | Modo | Request IPC | Response de éxito | Permiso | Scope |
| --- | --- | --- | --- | --- | --- |
| `lab_list_branches` | Laboratorio | `{}` | `BranchDto[]` | `branch.read` | Organización del principal |
| `lab_list_categories` | Laboratorio | `{ "request": { "limit": 20, "offset": 0 } }` | `CategoryDto[]` | `category.read` | Organización del principal |
| `lab_list_products` | Laboratorio | `{ "request": { "limit": 20, "offset": 0 } }` | `ProductDto_Serialize[]` | `product.read` | Organización del principal |
| `lab_create_product` | Laboratorio | `{ "request": { "sku", "name", "unitPrice", "categoryId?", "barcode?", "description?" } }` | `CreatedProductDto` | `product.create` | Organización del principal |
| `installation_status` | Instalación | `{}` | `InstallationStatusDto` | Ninguno (sin datos de negocio) | Instalación local |

`CreatedProductDto` devuelve `id`, `sku`, `name`, `unitPrice` exacto, `categoryId`
y `version`. La escritura es la única mutación expuesta y **solo existe en el
laboratorio**: la auditoría `product.created` se escribe en la misma transacción y
el actor se deriva del principal, nunca del request.

`InstallationStatusDto`: `state` (`FIRST_INSTALL`/`REOPENED`), `schemaVersion`
y `appliedMigrations`. No contiene rutas, usuarios, sesiones ni precios. El comando
de un modo responde `OPERATION_NOT_AVAILABLE` en el otro, sin consultar SQL.

Estado: **EXPOSED_VIA_IPC en aplicación de escritorio**. No son IPC comercial ni
`EXPOSED_VIA_HTTP`. El catálogo generado 1W sigue inventariando casos de
aplicación, no este registro de comandos.

`limit`: entero 1–100; `offset`: entero u32 desde cero. Los listados de catálogo
son ventanas por offset, **sin total/páginas calculadas**. Sucursales devuelve el
listado scoped completo; su paginación de producto sigue pendiente. La entrada
administrativa de laboratorio consulta todos los estados, no solo ACTIVE.
`organizationId`, actor y material de sesión no forman parte del request.
Los campos adicionales de `PageRequest` se rechazan por Serde.

Los DTOs no incluyen hashes, credenciales, sesiones ni entidades SQLx. IDs: UUID
canónico en string. `unitPrice`: string decimal escala 4, nunca número JS.
`version`: string entero, no number. Estos cambios respecto a DTOs Java no
sustituyen consumidores existentes ni aprueban el futuro contrato de escritura
`expectedVersion`. Esta vertical no tiene CAS de escritura que reintentar.

## Consumir desde React

```ts
import { desktopService } from '../../shared/rust-local/desktop-client'

const status = await desktopService.installationStatus()
// { state: 'REOPENED', schemaVersion: 1, appliedMigrations: 1 }
const products = await desktopService.listProducts()
// products[0].unitPrice === '12.3456'; se conserva texto exacto.
const created = await desktopService.createProduct({
  sku: 'CAFE-2', name: 'Café doble', unitPrice: '9.9900',
})
// created.unitPrice === '9.9900'; requiere product.create
```

El uso real y compilado está en `DesktopShell.tsx`, `InstallationPage.tsx` y
`DesktopCatalogPage.tsx`. No repartir `invoke()` por componentes. Los bindings
generados envuelven el resultado en `{status:'ok', data}` o
`{status:'error',error}`; el servicio los traduce a datos o
`DesktopCommandError.code`.

| Código tipado | Condición | Tratamiento frontend |
| --- | --- | --- |
| `UNAUTHORIZED` | Sesión inválida/vencida/revocada o estado no válido | No reintentar automáticamente |
| `FORBIDDEN` | Sesión válida sin permiso | Mostrar acceso denegado |
| `VALIDATION_ERROR` | Página fuera del contrato o entrada de dominio inválida | Corregir request |
| `CONFLICT` | La entidad ya existe en el scope (SKU o código de barras) | Mostrar conflicto, no reintentar |
| `NOT_FOUND` | La categoría o entidad referenciada no existe en el scope | Corregir referencia |
| `OPERATION_NOT_AVAILABLE` | El comando no existe en el modo actual | No reintentar; el modo lo decide el proceso |
| `INTERNAL_ERROR` | Fallo interno o fallo técnico IPC | Mensaje seguro, sin SQL/stderr |

Los errores de deserialización y ACL de Tauri se normalizan como fallo técnico
por el servicio. No son automáticamente el `ApiError`/status HTTP de Java.

## Generación, pruebas y ejecución real

```sh
# Desde frontend/
pnpm generate:desktop-contracts
pnpm test:desktop-lab
pnpm test:desktop-installation
```

`generate:desktop-contracts` utiliza Specta a partir de firmas Rust reales.
No editar `desktop.generated.ts`. El snapshot Rust compara la salida regenerada
y comprueba todos los nombres de comando. Specta permanece en RC fijada;
TypeScript 6 compila el resultado. El runtime generado contiene un cast `any`
propio de Tauri Specta: la excepción ESLint se limita a ese archivo, sin relajar
el código de aplicación.

`test:desktop-lab` construye el frontend y el ejecutable y ejecuta **WebView2 real**.
Playwright verifica el render React de sucursal/categoría/producto y el precio,
actualiza, **crea un producto por el formulario**, comprueba el precio exacto
`9.9900` en la tabla y el `CONFLICT` al repetir el SKU. También comprueba página
inválida, rechazo de overposting y que el laboratorio no expone
`installation_status`.

`test:desktop-installation` arranca **dos veces** sobre el mismo directorio
temporal con `--installation`, verifica `Primera instalación` y después
`Instalación reabierta` con la misma versión de esquema, comprueba que el archivo
sobrevive al cierre y que el modo instalación no sirve catálogo sintético.

En ambos casos el puerto DevTools es efímero de loopback, sólo del proceso de
prueba; no se abre en el arranque manual. La limpieza sólo cierra el proceso
creado por la prueba.

Separación de evidencias:

- Vitest `DesktopCatalogPage.test.tsx`: presentación unitaria con servicio mock.
- `pnpm test:rust-contracts`: proceso Rust/JSON del laboratorio 1W, **no IPC**.
- `cargo test --locked` en `backend-rust/apps/pos-desktop`: dispatcher Tauri con
  MockRuntime, modos, persistencia por reapertura, rechazo de esquema posterior,
  snapshots y aislamiento por organización; **no GUI real**.
- `pnpm test:desktop-lab` y `pnpm test:desktop-installation`:
  React/WebView2/IPC/SQLite real, **E2E de escritorio**.
- Docker `contracts`: suite Linux headless; **no reemplaza** el E2E de escritorio.

El workspace gráfico es independiente para no introducir GTK/WebKit en los
crates headless. Ejecutar también allí fmt, Clippy con `-D warnings`, tests y
build. Windows es el entorno gráfico ejecutado; no se afirma E2E desktop Linux.

## Purchasing preparado (continuación 2C)

Existe un registro separado de 17 handlers con DTOs/bindings generados y servicio
TypeScript/Query preparado. **No se incluye en el dispatcher activo ni en las
ACLs**, no tiene constructor de estado productivo y no conecta pantallas
comerciales a fixtures. Contratos, pruebas y bloqueos:
[Purchasing IPC preparado](rust-purchasing-ipc.md).

## Bloqueos y siguiente vertical

No hay login comercial ni escritura IPC. D-AUTH, D-INST, D-CAT, D-CASH, D-SALES,
D-INV y ADR-0022 siguen sujetos al expediente canónico; D-AUTH-09 continúa
bloqueada y el paquete de firma está en
[`rust-increment-1y-approval-package.md`](../05-development/rust-increment-1y-approval-package.md).
No se promueve DDL candidato ni se resuelve SYSTEM por introducir un transporte.
Próxima vertical: sesión comercial autorizada y lectura de catálogo sobre la
instalación persistente; después escritura CAS con auditoría, sólo cuando el
bloque físico y de seguridad correspondiente esté aceptado. Spring Boot no se
retira.
