# Purchasing Rust — contratos IPC preparados (continuación 2C)

**No disponibles en producto ni en la ventana de laboratorio.** Estos handlers
compilan y delegan en los 17 casos de uso existentes, pero su registro está
separado del dispatcher activo. No tienen ACLs en `capabilities/main.json` y el
runtime no construye `PurchasingState`. No hay HTTP Rust ni Swagger de compras.
Java HTTP sigue siendo el backend operativo.

## Fuente verificable y disponibilidad

- Requests/responses: `backend-rust/apps/pos-desktop/src/purchasing/dto.rs`.
- Handlers/registro preparado: `purchasing/commands.rs` y `purchasing/mod.rs`.
- Bindings generados: `frontend/src/shared/rust-local/purchasing.generated.ts`.
- Generación: `pnpm generate:desktop-contracts` desde `frontend` (exporta los
  bindings activos y los preparados por separado).
- Servicio: `purchasing-service.ts`; el singleton `purchasingService` está
  **cerrado por defecto** y falla con `OPERATION_NOT_AVAILABLE` sin llamar IPC.
- Integración Query: `purchasing-queries.ts`. Las claves incluyen un identificador
  **no secreto** de sesión/epoch de caché y el scope de la consulta. No usar
  tokens/contraseñas como claves y vaciar las cachés al cambiar/revocar la sesión.

Estado de todas las filas: `IMPLEMENTED_IN_APPLICATION`, persistencia candidata,
`PREPARED_HANDLER`, `BLOCKED_FOR_PRODUCT`. **No** `EXPOSED_VIA_IPC`,
`INTEGRATED_WITH_REACT` comercial ni `PARITY_VERIFIED`.

## Operaciones y correspondencia Java → Application → comando preparado

Los nombres TypeScript camelCase se generan a partir de los siguientes comandos.
El detalle Java de las 17 operaciones está en
[la auditoría de Purchasing](../05-development/rust-purchasing-parity-2a.md).

| Operación Java / caso Rust | Comando preparado | Request Rust → response | Permiso | Scope |
| --- | --- | --- | --- | --- |
| Crear proveedor / `create_supplier` | `purchasing_create_supplier` | `SupplierInput` → `SupplierDto` | `purchasing.supplier.write` | Organización |
| Obtener proveedor / `get_supplier` | `purchasing_get_supplier` | `SupplierKey` → `SupplierDto` | `purchasing.supplier.read` | Organización |
| Listar proveedores / `list_suppliers` | `purchasing_list_suppliers` | `SupplierList` → `SupplierDto[]` | `purchasing.supplier.read` | Organización |
| Actualizar proveedor / `update_supplier` | `purchasing_update_supplier` | `SupplierUpdate` → `SupplierDto` | `purchasing.supplier.write` | Organización |
| Crear orden / `create_purchase_order` | `purchasing_create_order` | `OrderInput` → `OrderDto` | `purchasing.order.write` | Sucursal |
| Detalle / `get_purchase_order` | `purchasing_get_order` | `OrderKey` → `OrderDto` | `purchasing.order.read` | Sucursal |
| Listar / `list_purchase_orders` | `purchasing_list_orders` | `OrderList` → `OrderDto[]` | `purchasing.order.read` | Sucursal |
| Cambiar proveedor / `change_purchase_order_supplier` | `purchasing_change_supplier` | `SupplierChange` → `OrderDto` | `purchasing.order.write` | Sucursal |
| Añadir partida / `add_purchase_order_line` | `purchasing_add_line` | `LineAdd` → `OrderDto` | `purchasing.order.write` | Sucursal |
| Editar partida / `update_purchase_order_line` | `purchasing_update_line` | `LineUpdate` → `OrderDto` | `purchasing.order.write` | Sucursal |
| Retirar partida / `remove_purchase_order_line` | `purchasing_remove_line` | `LineRemove` → `OrderDto` | `purchasing.order.write` | Sucursal |
| Emitir / `issue_purchase_order` | `purchasing_issue_order` | `OrderTransition` → `OrderDto` | `purchasing.order.issue` | Sucursal |
| Cancelar / `cancel_purchase_order` | `purchasing_cancel_order` | `OrderTransition` → `OrderDto` | `purchasing.order.cancel` | Sucursal |
| Recibir / `post_purchase_receipt` | `purchasing_post_receipt` | `ReceiptInput` → `ReceiptDto` | `purchasing.receipt.write` | Sucursal |
| Detalle recepción / `get_purchase_receipt` | `purchasing_get_receipt` | `ReceiptKey` → `ReceiptDto` | `purchasing.receipt.read` | Sucursal |
| Listar recepciones / `list_purchase_receipts` | `purchasing_list_receipts` | `ReceiptList` → `ReceiptDto[]` | `purchasing.receipt.read` | Sucursal |
| Acumulado / `purchase_receipt_status` | `purchasing_receipt_status` | `OrderKey` → `ReceiptStatusDto[]` | `purchasing.receipt.read` | Sucursal |

No se acepta `actorUserId`, `organizationId`, sesión ni ruta SQLite en los DTOs.
La sesión reside en el backend; el principal se reconstruye mediante SQLite en
cada llamada. Seleccionar `branchId` no concede permisos ni cambia organización.

## Representación y validación

- JSON camelCase; requests rechazan campos desconocidos.
- IDs: UUID validados antes de invocar Application.
- `expectedVersion` y `version`: texto decimal entero canónico (`"0"`, `"1"`,
  etc.), no número JavaScript. Se rechazan signo, decimales, ceros iniciales y
  overflow de `u64`. Se conserva `"9007199254740993"` sin pérdida.
- Dinero: strings exactos; costo pactado escala 4, cantidad escala 6 y total
  derivado escala 10 según Application. No usar `Number`/`parseFloat` para calcular.
- `unitCost: null` = desconocido; `"0.0000"` = cero conocido. `total` y `lineTotal`
  permanecen `null` si falta algún costo. No es una valoración de recepción.
- Paginación: `limit` 1..100, `offset` no negativo. Las respuestas actuales son
  arrays; no se inventa un conteo total que Application no ofrece.
- Filtros: proveedor `ACTIVE`/`INACTIVE`; orden según `PurchaseOrderStatus` de Rust.
- `receivedAt`: segundos Unix representados como texto, **no** ISO 8601. Una
  adaptación de fecha para UI no debe confundirlo con milisegundos.
- `SupplierDto` devuelve el resumen disponible en Application; no devuelve todos
  los campos de contacto aceptados al crear/actualizar. Esto y la forma de página
  requieren revisión diferencial antes de sustituir un consumidor Java.

### Patches, CAS y reintentos

Los patches opcionales se representan explícitamente, sin depender de que serde
conserve `Option<Option<T>>` al leer JSON:

```json
{ "unitCost": { "operation": "keep" } }
```
```json
{ "unitCost": { "operation": "clear" } }
```
```json
{ "unitCost": { "operation": "set", "value": "0.0000" } }
```

Omitir el patch equivale a `keep`; enviar `null` como patch no es válido. En alta
de partida, en cambio, `unitCost` es el costo nullable, no un patch. Es una
representación IPC explícita y no un reemplazo silencioso del PATCH Java.

Edición de proveedor, cambio de proveedor, edición/retirada de partida,
emisión/cancelación y recepción usan `expectedVersion`. La adición de partida
reproduce la firma existente de Application: no recibe revisión del cliente y
conserva el CAS interno; no se anuncia como un CAS aportado por la UI.

Ante `CONFLICT`, refrescar el recurso y pedir reconciliación antes de reenviar una
escritura. No reintentar mutaciones automáticamente. La recepción admite repetir
la misma clave de idempotencia y payload según las reglas existentes; cambiar el
payload no debe interpretarse como una segunda recepción válida.

Comprobado en 2D: la equivalencia relevante incluye organización derivada del
principal, sucursal, orden y el conjunto de **IDs de partida + cantidades exactas**,
no solo cantidades. `expectedVersion` es una precondición del primer intento y no
impide recuperar el resultado ya confirmado de un replay idéntico. La clave sigue
siendo única **por organización**, no global: nunca permite leer o recibir una
orden ajena. Rechazos/replays no duplican receipt, ledger, auditoría o estado.
Un actor de otra organización con permiso propio también queda rechazado; la
prueba no se limita a un actor sin grants.

## Errores seguros

| Código | Significado |
| --- | --- |
| `UNAUTHORIZED` | Sesión inválida, inexistente, expirada o revocada; sin revelar existencia de usuario |
| `FORBIDDEN` | Permiso/scope insuficiente u ownership rechazado |
| `VALIDATION_ERROR` | DTO inválido, decimal/cantidad/página inválida o sobre-recepción |
| `NOT_FOUND` | Recurso no visible en el scope actual |
| `CONFLICT` | CAS, código duplicado, estado incompatible, proveedor inactivo o conflicto de idempotencia |
| `OPERATION_NOT_AVAILABLE` | Servicio/frontend no activado; el dispatcher activo no registra los comandos preparados |
| `INTERNAL_ERROR` | Fallo interno sin SQL, hashes, tokens ni detalles físicos |

El dispatcher real responde a un comando no registrado con rechazo del transporte,
no necesariamente con `OPERATION_NOT_AVAILABLE`; el servicio cerrado evita esa
llamada. No usar el mensaje de transporte como prueba de autenticación.

## Ejemplo tipado y preparación React

El patrón está compilado y probado en `purchasing-service.test.ts`:

```ts
import { purchasingService } from '../../shared/rust-local/purchasing-service'
import { purchasingQueries } from '../../shared/rust-local/purchasing-queries'

const options = purchasingQueries(purchasingService, 'cache-epoch-1')
  .orders({ branchId, limit: 20, offset: 0 })
// useQuery(options): enabled=false mientras la composición no esté autorizada.
```

No se integra ninguna pantalla comercial con datos sintéticos. La futura
composición autorizada podrá inyectar los comandos generados en
`createPurchasingService`; no basta con habilitarlo en JavaScript: antes deben
existir runtime/ACLs, sesión comercial y esquema aprobado.

## Pruebas y alcance

- `purchasing::tests`: las 17 funciones handler se ejercitan directamente con
  estado Tauri y SQLite candidato del **fixture existente**, sin un nuevo actor.
  Prueba proveedor/CAS, órdenes, costos nulos/cero, emisión/cancelación,
  recepciones/idempotencia, sobre-recepción, scope, permisos y revocación.
- La prueba compara bindings regenerados y comprueba que no haya comandos de
  Purchasing en el registro activo. También verifica nombres de la tabla anterior.
- `purchasing-service.test.ts`: comandos generados con `invoke` simulado, exactitud,
  errores seguros, modo cerrado y QueryClient. **No es IPC real ni E2E desktop.**
- Los E2E gráficos existentes siguen validando los comandos activos, no Purchasing.
- No se ejecuta aún un E2E comercial ni una comparación diferencial completa
  Java/Rust. `0016`, A/B/C, D-AUTH-09 y decisiones económicas siguen bloqueando
  la activación; CC21-03 no autoriza todo el esquema ni costo efectivo.
