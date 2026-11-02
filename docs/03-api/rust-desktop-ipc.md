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
| `bootstrap_status` | Instalación | `{}` | `BootstrapStatusDto` | Ninguno (solo estado) | Instalación local |
| `bootstrap_install` | Instalación | `{ "request": { "businessName", "businessCode?", "administratorName", "email", "password", "currency?" } }` | vacío | Autorización nativa viva | Instalación local |
| `auth_login` | Instalación | `{ "request": { "email", "password" } }` | `SessionIdentityDto` | Ninguno (autentica) | Instalación local |
| `auth_me` | Instalación | `{}` | `SessionIdentityDto` | Sesión vigente | Sesión nativa |
| `auth_logout` | Instalación | `{}` | vacío | Sesión vigente | Sesión nativa |
| `auth_logout_all` | Instalación | `{}` | `LogoutAllDto` | Sesión vigente | Usuario de la sesión |
| `auth_select_branch` | Instalación | `{ "request": { "branchId" } }` | `SessionIdentityDto` | `branch.read` en el scope resultante | Organización del principal |
| `auth_list_branches` | Instalación | `{}` | `BranchDto[]` | `branch.read` | Organización del principal |
| `catalog_list_categories` | Instalación | `{ "request": { "limit", "offset" } }` | `CategoryDto[]` | `category.read` | Organización del principal |
| `catalog_list_products` | Instalación | `{ "request": { "limit", "offset" } }` | `ProductDto_Serialize[]` | `product.read` | Organización del principal |
| `catalog_get_product` | Instalación | `{ "request": { "id" } }` | `ProductDto_Serialize` | `product.read` | Organización del principal |
| `catalog_create_category` | Instalación | `{ "request": { "code", "name" } }` | `CategoryDto` | `category.create` | Organización del principal |
| `catalog_create_product` | Instalación | `{ "request": { "sku", "name", "unitPrice", "categoryId?", "barcode?", "description?" } }` | `CreatedProductDto` | `product.create` | Organización del principal |
| `catalog_update_product` | Instalación | `{ "request": { "productId", "expectedVersion", "name?", "unitPrice?", "categoryId?", "status?" } }` | `ProductDto_Serialize` | `product.update` | Organización del principal |
| `organization_get` | Instalación | `{}` | `OrganizationDto` | `organization.read` | Scope propio del principal |
| `organization_update` | Instalación | `{ "request": { "expectedRevision", "name?", "timezone?", "locale?", "negativeStockPolicy?" } }` | `OrganizationDto` | `organization.update` | Scope propio del principal |
| `branch_list` | Instalación | `{}` | `BranchAdminDto[]` | `branch.read` | Organización del principal |
| `branch_get` | Instalación | `{ "request": { "id" } }` | `BranchAdminDto` | `branch.read` | Organización del principal |
| `branch_create` | Instalación | `{ "request": { "code", "name", "timezone?" } }` | `BranchAdminDto` | `branch.create` | Organización del principal |
| `branch_update` | Instalación | `{ "request": { "branchId", "expectedRevision", "name?", "timezone": { "mode", "timezone?" } } }` | `BranchAdminDto` | `branch.update` | Organización del principal |
| `user_list` | Instalación | `{}` | `UserDto[]` | `user.read` | Organización del principal |
| `user_get` | Instalación | `{ "request": { "id" } }` | `UserDto` | `user.read` | Organización del principal |
| `user_create` | Instalación | `{ "request": { "displayName", "email", "password" } }` | `UserDto` | `user.create` | Organización del principal |
| `user_update` | Instalación | `{ "request": { "userId", "expectedRevision", "displayName?", "email?", "status?" } }` | `UserDto` | `user.update` | Organización del principal |
| `user_set_roles` | Instalación | `{ "request": { "userId", "roleIds" } }` | `UserDto` | `role.assign` | Organización del principal |
| `role_list` | Instalación | `{}` | `RoleDto[]` | `role.read` | Organización del principal |
| `role_get` | Instalación | `{ "request": { "id" } }` | `RoleDto` | `role.read` | Organización del principal |
| `role_create` | Instalación | `{ "request": { "code", "name", "permissionCodes" } }` | `RoleDto` | `role.create` (+ `role.permissions.manage` si concede permisos) | Organización del principal |
| `role_update` | Instalación | `{ "request": { "roleId", "expectedRevision", "name?", "status?" } }` | `RoleDto` | `role.update` | Organización del principal |
| `role_set_permissions` | Instalación | `{ "request": { "roleId", "expectedRevision", "permissionCodes" } }` | `RoleDto` | `role.permissions.manage` | Organización del principal |
| `permission_list` | Instalación | `{}` | `PermissionDto[]` | `permission.read` | Sistema |
| `membership_list` | Instalación | `{}` | `MembershipDto[]` | `branch.membership.read` | Organización del principal |
| `membership_create` | Instalación | `{ "request": { "branchId", "userId", "roleIds" } }` | `MembershipDto` | `branch.membership.create` (+ `branch.membership.roles.manage` si asigna roles) | Sucursal validada en la organización |
| `membership_update` | Instalación | `{ "request": { "membershipId", "expectedRevision", "status?", "roleIds?" } }` | `MembershipDto` | `branch.membership.update` (+ `branch.membership.roles.manage` al cambiar roles) | Sucursal de la membresía |
| `cash_register_list` | Instalación | `{ "request": { "status?", "limit", "offset" } }` | `CashRegisterDto[]` | `cash-register.read` | Sucursal seleccionada |
| `cash_register_get` | Instalación | `{ "request": { "id" } }` | `CashRegisterDto` | `cash-register.read` | Sucursal seleccionada |
| `cash_register_create` | Instalación | `{ "request": { "code", "name" } }` | `CashRegisterDto` | `cash-register.create` | Sucursal seleccionada |
| `cash_register_update` | Instalación | `{ "request": { "cashRegisterId", "expectedRevision", "name?", "status?" } }` | `CashRegisterDto` | `cash-register.update` | Sucursal seleccionada |
| `cash_shift_open` | Instalación | `{ "request": { "cashRegisterId", "openingAmount" } }` | `CashShiftDto` | `cash-shift.open` | Sucursal seleccionada |
| `cash_shift_get_open` | Instalación | `{ "request": { "id" } }` | `CashShiftDto \| null` | `cash-shift.read` | Sucursal seleccionada |
| `cash_shift_get` | Instalación | `{ "request": { "cashRegisterId", "cashShiftId" } }` | `CashShiftDto` | `cash-shift.read` | Sucursal seleccionada |
| `cash_shift_list` | Instalación | `{ "request": { "cashRegisterId", "status?", "limit", "offset" } }` | `CashShiftDto[]` | `cash-shift.read` | Sucursal seleccionada |
| `cash_shift_close` | Instalación | `{ "request": { "cashRegisterId", "cashShiftId", "expectedRevision", "closingDeclaredAmount" } }` | `CashShiftDto` | `cash-shift.close` | Sucursal seleccionada; scope de recuperación solo aquí |

### Primera instalación

`bootstrap_status` no muta nada. `bootstrap_install` solo actúa sobre un derecho
vivo leído del archivo privado del directorio de instalación; el token no aparece
en ningún request, respuesta, log ni binding. La organización, el administrador,
la credencial y los 59 grants se crean en la misma UoW que consume la
autorización. Diseño en [ADR-0028](../01-architecture/adr/0028-guided-first-installation.md).

### Catálogo

Los seis comandos `catalog_*` revalidan la sesión nativa en cada llamada y
resuelven organización y permisos en SQLite. El cliente aporta solo datos de
negocio y la versión que leyó; el precio viaja como texto decimal de escala 4.

### Configuración del negocio

Cinco comandos administran la organización y su sucursal. La organización **nunca**
llega por IPC: se deriva del `VerifiedPrincipal`, y el permiso se resuelve contra
el scope exacto ya resuelto en SQLite. Los DTO declaran `denyUnknownFields`, así
que un campo ajeno al contrato es un error de validación y no un cambio ignorado.

Campos editables y campos que el contrato **no puede expresar**:

| Campo | Estado | Motivo |
| --- | --- | --- |
| `organization.name` | editable | Visible sin efecto operativo |
| `organization.timezone` | editable | Base de la herencia de sucursal |
| `organization.locale` | editable | Visible sin efecto operativo |
| `organization.negativeStockPolicy` | editable | Contrato aprobado en ADR-0013 |
| `organization.status` | solo lectura | Sin contrato de baja ni recuperación |
| `organization.currency` | solo lectura | Reinterpretaría importes persistidos |
| `branch.name` | editable | Visible sin efecto operativo |
| `branch.timezone` (`mode`) | editable | Triestado explícito |
| `branch.status` | solo lectura | Desactivar la única sucursal dejaría la instalación inoperable |
| `branch.code`, `branch.id` | inmutables | Identidad del agregado |

`branch_update.timezone` usa un triestado explícito que no se colapsa en `null`:

| `mode` | `timezone` | Efecto |
| --- | --- | --- |
| `KEEP` | debe faltar | Conserva la configuración |
| `SET` | obligatorio | Define zona propia |
| `INHERIT` | debe faltar | Elimina el override y hereda |

`BranchAdminDto` incluye `effectiveTimezone` y `inheritsTimezone` ya resueltos
por el dominio, de modo que la interfaz no reimplementa la herencia.

Las revisiones viajan como **texto decimal exacto** (`"4"`), nunca como número de
JavaScript. Una revisión obsoleta responde `CONFLICT`; un patch sin cambios
materiales no incrementa la revisión ni genera auditoría.

**Capacidad de Free Local.** La edición admite una única sucursal. La primera
alta configura la sucursal inicial de la instalación; cualquier intento posterior
responde `CONFLICT` desde la capa de aplicación aunque el principal tenga
`branch.create`. No es una restricción de interfaz.

### Administración de identidad

Catorce comandos administran usuarios, roles y accesos por sucursal. La
organización, el actor y los permisos **nunca** llegan por IPC: se derivan del
`VerifiedPrincipal` y se revalidan en SQLite en cada llamada.

| Contrato | Efecto |
| --- | --- |
| `user_create` | Crea `pos_user` + `local_credential` Argon2id y audita `user.created` en la misma UoW |
| `user_update` | CAS sobre `pos_user.version`; desactivar revoca las sesiones del usuario |
| `user_set_roles` | Reemplaza `user_organization_role`; prohibido sobre el propio actor |
| `role_create` / `role_update` | Código inmutable y único; nombre y estado editables |
| `role_set_permissions` | Reemplaza `role_permission` validando el catálogo del sistema |
| `membership_create` / `membership_update` | Alta y mantenimiento de `branch_membership` y sus roles |

La contraseña inicial se envía **una sola vez**, se deriva en Rust y nunca se
vuelve a leer. Ni el handle de sesión, ni el hash de la credencial, ni el digest
de sesión aparecen en ninguna respuesta.

**Reglas de continuidad.** Un administrador no puede cambiar sus propios roles ni
desactivar su cuenta, y ninguna operación puede dejar la organización sin un
usuario ACTIVO con rol ACTIVO que conceda `user.update`. El rechazo es
`FORBIDDEN` para la auto-modificación y `CONFLICT` para la pérdida de
administración.

**Alcance de sucursal.** Una membresía ACTIVA con rol ACTIVO es la única vía por
la que `auth_select_branch` obtiene permisos de sucursal. Crear la membresía y
dotarla de roles es lo que habilita operar en caja, ventas e inventario.

Diseño y consecuencias en
[ADR-0030](../01-architecture/adr/0030-local-identity-administration.md).

### Cajas y turnos

Nueve comandos activan el contrato de `0009`/`0010` sobre la sucursal
seleccionada. La sucursal **no** viaja en la petición: el proceso Rust la custodia
desde `auth_select_branch` y la aplica en cada revalidación
([ADR-0031](../01-architecture/adr-0031-native-branch-selection.md)).

| Dato | Transporte | Motivo |
| --- | --- | --- |
| `openingAmount`, `closingDeclaredAmount` | texto decimal exacto (máx. 4 decimales) | Nunca se usa coma flotante |
| `expectedRevision` | texto decimal | Revisión de 64 bits |
| `openedAt`, `closedAt` | texto decimal de segundos Unix | El renderer formatea, no rederiva |
| `branchId`, `organizationId` | ausentes | Salen del scope verificado |

Rechazos públicos: `cash-register.create` sin permiso → `FORBIDDEN`; código
duplicado, caja con turno abierto, turno ya cerrado, ventas `OPEN` o revisión
obsoleta → `CONFLICT`; caja o turno de otra sucursal → `NOT_FOUND`; importe con
más de cuatro decimales o página fuera de rango → `VALIDATION_ERROR`.

El cierre es el único comando que admite el scope de recuperación del contrato:
puede cerrar un turno histórico sobre una caja o sucursal ya inactiva. La apertura
y la administración de cajas no lo admiten.

**Fuera de alcance del contrato actual**: movimientos de efectivo, arqueos y
totales esperados. No se calculan ni se simulan.


### Autenticación Local

`auth_login` es el único comando que acepta una contraseña. El proceso Rust
verifica Argon2id y **custodia el handle de sesión en memoria nativa**: la
respuesta solo contiene identidad y permisos. Los comandos posteriores usan ese
handle sin que el renderer pueda leerlo ni guardarlo; `localStorage`,
`sessionStorage` y SQLite nunca reciben el secreto (solo su digest SHA-256).

`SessionIdentityDto`: `userId`, `organizationId`, `displayName`, `email`,
`branchId` opcional y `permissions` efectivos resueltos en SQLite. No incluye
handle, digest, hash de credencial ni tokens.

Rechazos: usuario inexistente, contraseña errónea, cuenta inactiva, credencial
bloqueada o reset pendiente responden todos `UNAUTHORIZED`, sin distinguir el
motivo. Un correo mal formado también responde `UNAUTHORIZED`. Sin sesión, los
demás comandos `auth_*` responden `UNAUTHORIZED`; en laboratorio responden
`OPERATION_NOT_AVAILABLE`.

`auth_select_branch` exige que la sucursal exista en la organización del
principal y esté `ACTIVE`. Una sucursal de otra organización responde `NOT_FOUND`
sin revelar su existencia; una inactiva o no descubrible responde `FORBIDDEN`.

La sesión no sobrevive al cierre del proceso: al reabrir hay que autenticarse de
nuevo. El usuario, sus permisos y su auditoría permanecen en SQLite. Diseño
completo en [ADR-0027](../01-architecture/adr/0027-local-authentication.md).

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

La organización y su sucursal ya se administran por IPC con sesión real, CAS y
auditoría transaccional. Siguen abiertos: administración de usuarios y
membresías por IPC, caja/turnos, ventas, inventario, compras y E2E gráfico de
WebView2. D-AUTH-09 continúa sin ratificar y el gate normativo sigue `BLOCKED`
para `main`; nada de eso cambia por tener este vertical verde. Spring Boot no se
retira.
