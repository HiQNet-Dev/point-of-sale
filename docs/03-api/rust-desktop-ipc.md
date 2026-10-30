# Rust desktop — primera vertical IPC real (1X)

**Solo laboratorio, no productivo.** Java HTTP/OpenAPI y las pantallas existentes
siguen operativos. No hay servidor HTTP Rust. Ver [ADR-0023](../01-architecture/adr/0023-tauri-readonly-laboratory.md).

## Levantar la aplicación para probarla

En Windows con Rust 1.99.1, herramientas MSVC y WebView2 instalados:

```sh
cd frontend
pnpm install --frozen-lockfile
pnpm desktop:lab
```

El comando construye una entrada React separada y abre una ventana Tauri. No
necesita Spring Boot, PostgreSQL, Docker ni Vite escuchando. Muestra sucursales,
categorías y productos desde **SQLite real**, con el precio exacto `12.3456`.
El botón Actualizar vuelve a ejecutar los casos de uso por IPC.

La base se crea en un directorio temporal nuevo para cada arranque. No se acepta
una ruta de cliente, no hay bootstrap público ni capacidad de escritura desde
la pantalla. **No es aún una instalación comercial durable**. La fixture prepara
bootstrap, sucursal, roles, sesión y receta; utiliza un reloj sintético fijo.
No demuestra login comercial ni expiración basada en el reloj de pared.

## Camino ejecutado

```text
DesktopCatalogPage (React + TanStack Query)
 → desktopCatalogService
 → desktop.generated.ts (Tauri Specta)
 → IPC Tauri / WebView2
 → commands.rs (solo adaptación)
 → composition.rs (principal DB-authoritative + composición)
 → Application y puertos
 → adaptadores SQLite existentes
 → DTO tipado → React
```

La fixture de `pos-lab` se comparte como biblioteca; no se ha copiado el flujo
comercial al handler. `pos-local` conserva exclusivamente las migraciones
aprobadas, sin cargar `proposals/`. No se activan servicios candidatos en el
composition root de producto.

## Contratos registrados

Fuente verificable: `tools/desktop-lab/src/commands.rs` + `dto.rs`, relativa a
`backend-rust/`. El mismo registro configura dispatcher y generación de bindings.

| Comando real | Request IPC | Response de éxito | Permiso | Scope |
| --- | --- | --- | --- | --- |
| `lab_list_branches` | `{}` | `BranchDto[]` | `branch.read` | Organización del principal |
| `lab_list_categories` | `{ "request": { "limit": 20, "offset": 0 } }` | `CategoryDto[]` | `category.read` | Organización del principal |
| `lab_list_products` | `{ "request": { "limit": 20, "offset": 0 } }` | `ProductDto_Serialize[]` | `product.read` | Organización del principal |

Estado: **EXPOSED_VIA_IPC en app de laboratorio** y consumidos por su pantalla
React. No son IPC comercial ni `EXPOSED_VIA_HTTP`. El catálogo generado 1W sigue
inventariando casos de aplicación, no este registro de comandos de laboratorio.

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
import { desktopCatalogService } from '../../shared/rust-local/desktop-client'

const products = await desktopCatalogService.listProducts()
// products[0].unitPrice === '12.3456'; se conserva texto exacto.
```

El uso real y compilado está en `DesktopCatalogPage.tsx`. No repartir `invoke()`
por componentes. Los bindings generados envuelven el resultado en `{status:'ok',
data}` o `{status:'error',error}`; el servicio los traduce a datos o
`DesktopCommandError.code`.

| Código tipado | Condición | Tratamiento frontend |
| --- | --- | --- |
| `UNAUTHORIZED` | Sesión inválida/vencida/revocada o estado no válido | No reintentar automáticamente |
| `FORBIDDEN` | Sesión válida sin permiso | Mostrar acceso denegado |
| `VALIDATION_ERROR` | Página fuera del contrato | Corregir request |
| `INTERNAL_ERROR` | Fallo interno o fallo técnico IPC | Mensaje seguro, sin SQL/stderr |

Los errores de deserialización y ACL de Tauri se normalizan como fallo técnico
por el servicio. No son automáticamente el `ApiError`/status HTTP de Java.

## Generación, pruebas y ejecución real

```sh
# Desde frontend/
pnpm generate:desktop-contracts
pnpm test:desktop-lab
```

`generate:desktop-contracts` utiliza Specta a partir de firmas Rust reales.
No editar `desktop.generated.ts`. El snapshot Rust compara la salida regenerada
y comprueba todos los nombres de comando. Specta permanece en RC fijada;
TypeScript 6 compila el resultado. El runtime generado contiene un cast `any`
propio de Tauri Specta: la excepción ESLint se limita a ese archivo, sin relajar
el código de aplicación.

`test:desktop-lab` construye el frontend y el ejecutable y ejecuta **WebView2 real**.
Playwright verifica el render React de sucursal/categoría/producto y el precio,
actualiza y comprueba página inválida y rechazo de overposting. El puerto
DevTools es efímero de loopback, solo del proceso de prueba; no se abre en el
arranque manual. La limpieza solo cierra el proceso creado por la prueba.

Separación de evidencias:

- Vitest `DesktopCatalogPage.test.tsx`: presentación unitaria con servicio mock.
- `pnpm test:rust-contracts`: proceso Rust/JSON del laboratorio 1W, **no IPC**.
- `cargo test --locked` en `backend-rust/tools/desktop-lab`: dispatcher Tauri con
  MockRuntime, snapshots y seguridad SQLite; **no GUI real**.
- `pnpm test:desktop-lab`: React/WebView2/IPC/SQLite real, **E2E de escritorio**.
- Docker `contracts`: suite Linux headless; **no reemplaza** el E2E de escritorio.

El workspace gráfico es independiente para no introducir GTK/WebKit en los
crates headless. Ejecutar también allí fmt, Clippy con `-D warnings`, tests y
build. Windows es el entorno gráfico ejecutado; no se afirma E2E desktop Linux.

## Bloqueos y siguiente vertical

No hay escritura IPC ni autenticación comercial. D-AUTH, D-INST, D-CAT, D-CASH,
D-SALES, D-INV y ADR-0022 siguen sujetos al expediente canónico; D-AUTH-09
continúa bloqueada. No se promueve DDL candidato ni se resuelve SYSTEM por
introducir un transporte. Próxima vertical: sesión comercial autorizada y
lectura de catálogo en el composition root canónico; después escritura CAS con
auditoría, solo cuando el bloque físico y de seguridad correspondiente esté
aceptado. Spring Boot no se retira.
