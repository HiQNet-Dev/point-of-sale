# Rust Local — contratos verificables (1W)

## Alcance histórico de la vertical 1W

**1W no registra comandos Tauri ni endpoints Rust HTTP.** Para los tres comandos
IPC reales de laboratorio añadidos en 1X, ver [Rust desktop IPC](rust-desktop-ipc.md). La referencia HTTP
Java sigue siendo Springdoc `/v3/api-docs`; Swagger y su flujo Authorize no cambian.
El cliente Java existente no se sustituye. No hay compatibilidad JSON implícita:
los importes Rust de esta vertical son **strings exactos**, no `number`.

La arquitectura vigente contempla Tauri/IPC **cuando se apruebe**. Este incremento
no aprueba su activación, no añade sockets, Tauri, Specta, Axum ni Swagger a Rust.
En 1W no se validó una combinación Tauri/Specta; la combinación comprobada para
el laboratorio 1X está documentada por separado.
El catálogo no representa un servidor ni un registro de comandos de escritorio.

## Descubrimiento por operación

[`rust-operations.generated.json`](rust-operations.generated.json) inventaría las
**32 operaciones async actuales** de catálogo, caja, ventas, inventario y checkout:
identificador estable `rust.<contexto>.<método>`, permiso canónico, scope del
principal, firma interna y ubicación de DTOs. Se genera desde los casos de uso y
el enum canónico de permisos, no desde SQLx. Un test falla si cambia el inventario.

Cada entrada es `IMPLEMENTED_IN_APPLICATION`, **no** `EXPOSED_VIA_IPC`,
`EXPOSED_VIA_HTTP` ni `INTEGRATED_WITH_REACT`. `transportRequest`,
`transportResponse`, `ipcCommand` y `httpRoute` son null deliberadamente: todavía
no existe un contrato de transporte para esas operaciones individuales. Sus
firmas Rust no deben convertirse por intuición en JSON público.

| Contexto | Estado verificable / qué falta para frontend |
| --- | --- |
| Bootstrap | Capacidad candidata ejercitada en laboratorio; sin handler público |
| Auth y sesiones | Principal y RBAC DB-authoritative; sesión del escenario insertada como fixture, **sin login comercial** |
| Organización/sucursales | Casos existentes; sucursal del laboratorio preparada mediante fixture, no comando público |
| Usuarios/roles/RBAC | Preparación sintética y resolución en SQLite; administración no expuesta |
| Categorías/productos | 8 casos de uso inventariados; creación ejercitada en laboratorio |
| Modificadores | Dominio/esquema candidato; sin contratos administrativos generados |
| Cajas/turnos | 9 casos inventariados; creación, apertura, guard y cierre ejercitados |
| Inventario | 7 casos inventariados; apertura y consumo ejercitados |
| Recetas | Fixture SQL del escenario, no administración productiva |
| Ventas/partidas | 7 casos inventariados; creación y partida ejercitadas |
| Checkout/pagos | Caso integral inventariado y ejecutado contra candidato; no endpoint |
| Proveedores/compras/recepciones | No expuestos; no se afirma paridad de persistencia o transporte |
| Auditoría | Writer transaccional utilizado; consulta administrativa no expuesta |

**Pendiente:** DTOs públicos por operación, ejemplos y errores exhaustivos,
contratos de paginación, fechas, modificadores y compras. El inventario interno
no sustituye ese trabajo ni declara completada la API comercial.

## Vertical disponible: `lab.runScenario`

Estado: `AVAILABLE_IN_LAB`. Es el identificador del adaptador de pruebas
TypeScript, **no un comando Tauri**. No recibe organización, actor, credenciales,
rutas de archivo ni SQL desde React. Request lógico: `{}`. El runner crea un
directorio temporal privado y suministra su ruta al proceso Rust. Nunca utilizar
este runner desde una aplicación web ni conectarlo a bases existentes.

Fuente de verdad del DTO: `backend-rust/tools/pos-lab/src/contract.rs`.
Genera `frontend/src/shared/rust-local/generated.ts`: interfaz TypeScript y JSON
Schema draft 2020-12, campos obligatorios y rechazo de campos adicionales. No hay
entidades SQLx ni secretos en la respuesta.

Ejemplo real de respuesta validada por la prueba:

```json
{
  "productive": false,
  "scope": "synthetic_sqlite_candidate_schema",
  "saleId": "00000000-0000-0000-0000-000000000000",
  "saleTotal": "24.6912",
  "changeAmount": "5.3088",
  "saleStatus": "COMPLETED",
  "cashShiftStatus": "CLOSED",
  "inventoryBalance": "500.000000",
  "openSalePreventedClose": true
}
```

El ID ilustrado es un UUID de formato válido; el ID real lo genera el escenario.
Importes: escala 4; cantidades de inventario: escala 6. Nunca usar `Number`,
`parseFloat` ni redondeo para cálculos financieros. El esquema generado valida
estructura/tipos; el cliente añade validación de formato decimal, UUID, estado
y marca no productiva. El esquema por sí solo no expresa todas esas invariantes.

## Ejecutar el recorrido Rust → TypeScript

Requisitos: toolchain de `backend-rust/rust-toolchain.toml`, Node y pnpm según
`frontend/package.json`; dependencias frontend instaladas con lockfile.

```sh
cd frontend
pnpm install --frozen-lockfile
pnpm test:rust-contracts
```

El comando funciona sin variables de entorno específicas del shell. La suite
ordinaria `pnpm test` omite las tres pruebas que exigen Cargo; **no sustituye**
`test:rust-contracts`. El escenario funcional se valida primero en **Docker
local**, no se lanza en GitHub Actions. El workflow `Rust Local Contracts` queda
limitado a sincronización de artefactos generados y typecheck; no sustituye ni
debilita CI Gate, DDL13 o require-complete.

La prueba implementa `LabTransport` usando el binario Rust real, no un mock:

```ts
import { createLabClient } from './client'

// transport es un adaptador aislado inyectado por el runner, no invoke de Tauri.
const client = createLabClient(transport)
const report = await client.runScenario()
// report.saleTotal: string; report.productive: boolean (validado como false).
```

Este fragmento se materializa y compila en `client.test.ts`; ninguna pantalla
React actual se conecta todavía a esta superficie.

El escenario confirma casos de aplicación reales: categoría, producto, caja,
turno, venta, partida, total, guard de cierre, pago, consumo de inventario,
COMPLETED, cierre y reapertura durable. **Bootstrap, sesión, sucursal y receta
incluyen preparación sintética; no demuestra autenticación comercial completa.**
Las migraciones candidatas siguen fuera del migrador productivo.

## Linux con Docker local (sin runners de GitHub)

Desde la raíz del repositorio:

```sh
docker compose -f docker-compose.rust.yml --profile contracts build contracts
docker compose -f docker-compose.rust.yml --profile contracts run --rm --no-deps contracts
```

El servicio opt-in `contracts` usa `backend-rust/docker/contracts.Dockerfile`.
Compila los targets Rust y prepara dependencias frontend durante el build;
**la construcción requiere acceso a los registros de imágenes y paquetes**.
La ejecución posterior no tiene red (`network_mode: none`), corre como UID/GID
10001, no tiene puertos ni bind mounts ni acceso a los volúmenes `pos-local-data`
o `pos-lab-data`. Las bases temporales se crean y eliminan dentro del contenedor.
El contexto excluye `.env`, claves privadas, bases SQLite y artefactos locales.

Es una imagen de pruebas con filesystem escribible para Cargo/Vite, no el runtime
del producto. El servicio `pos-local` conserva su filesystem read-only y su
volumen privado; el Compose Java no cambia. No es necesario ejecutar `up`,
`down -v` ni borrar ningún volumen para estas pruebas.

El script falla en el primer error y ejecuta fmt, Clippy con `-D warnings`, tests
y build de todo el workspace, lint/typecheck/tests/build frontend y las tres
pruebas de contrato contra Rust/SQLite real. Los contratos generados también se
comparan con la fuente Rust. La imagen no modifica archivos del checkout.

**Evidencia local ejecutada en Docker Desktop, Linux/amd64:** build PASS;
script completo PASS; UID/GID 10001; frontend 209 tests PASS y tres integraciones
PASS por separado. El build Vite mantiene una advertencia no bloqueante por
bundle de más de 500 kB. Esto no acredita PostgreSQL 17 Migration Lab completo,
Playwright, aprobación física, despliegue productivo ni CI remoto verde.
La ejecución GitHub `41101144018` se canceló antes de obtener runner; no se
utiliza como evidencia de validación Linux.

## Regenerar y verificar

Desde `backend-rust/`:

```sh
cargo run --locked -q -p pos-lab -- --export-contract > ../frontend/src/shared/rust-local/generated.ts
cargo run --locked -q -p pos-lab -- --export-operations > ../docs/03-api/rust-operations.generated.json
cargo test --locked -p pos-lab
```

No editar los artefactos generados. Cambiar el DTO/inventario exige regenerarlos
y revisar el diff. La macro soporta deliberadamente solo strings y booleanos;
un nuevo tipo requiere extender y probar explícitamente su representación.
No es un generador general de bindings Tauri.

## Autorización, errores y concurrencia

Las operaciones comerciales siguen exigiendo principal verificado, scope y
permisos resueltos en SQLite. El cliente no los concede. La fixture posee los
permisos necesarios; **no es allow-all ni un endpoint de provisioning**.
El guard se comprueba por `CashShiftHasOpenSales`, no por cualquier error.

no hLos códigos `INVALID_LAB_RESPONSE` y `LAB_SCENARIO_FAILED` se generan desde el
enum Rust, junto con el tipo `LabError`. `RustLocalError.code` permite manejarlos
sin comparar mensajes. El primero indica incompatibilidad de estructura,
precisión, scope o estado. El segundo identifica el fallo del proceso aislado:
el cliente descarta diagnósticos internos y Rust emite un error genérico sin SQL.
No reenviar stderr o errores SQL a pantallas. No hay aún catálogo público Rust de errores
por operación ni equivalencia automática con códigos/status Java.

No reintentar automáticamente mutaciones ni ejecutar de nuevo el escenario
sobre un archivo existente: Rust lo rechaza sin borrarlo. En futuros comandos
CAS, releer el recurso y solicitar resolución del usuario antes de reenviar un
`expectedVersion` nuevo. Las transacciones, auditoría y serialización crítica
siguen siendo responsabilidad de Application y de una única UoW SQLite.

## Bloqueos de producto

No se aprueban D-AUTH, D-INST, D-CAT, D-CASH, D-SALES, D-INV ni ADR-0022.
D-AUTH-09 comercial continúa bloqueada. Esta vertical no activa DDL, login,
IPC, HTTP, integración React ni cutover. El siguiente paso de transporte exige
aceptación vigente, composition root autorizado y mapeo público de errores;
reutilizará estos casos de uso, no una segunda implementación empresarial.
