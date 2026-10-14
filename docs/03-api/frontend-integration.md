# Integración desde el Frontend

Guía práctica para consumir la API desde la SPA (React + TypeScript). La
especificación en Swagger es la fuente de verdad; esta guía cubre lo que suele
generar dudas al integrar.

## Base URL y CORS

| Entorno | Base URL |
| --- | --- |
| Desarrollo (Vite) | `http://localhost:8080/api/v1` (`VITE_API_BASE_URL`) |
| Producción / same-origin | `/api/v1` (relativo) |

- En desarrollo el backend permite CORS desde `http://localhost:5173`
  (`CORS_ALLOWED_ORIGINS`), sin wildcard.
- Prefijo de versión fijo: **`/api/v1`**. No hay otras versiones todavía.

## Cabeceras

```http
Authorization: Bearer <accessToken>
Content-Type: application/json
```

## Ejemplo (fetch)

```ts
const res = await fetch(`${import.meta.env.VITE_API_BASE_URL}/organizations/${orgId}/products?page=0&size=20`, {
  headers: { Authorization: `Bearer ${accessToken}` },
});
if (!res.ok) {
  const error: ApiError = await res.json(); // { timestamp, status, code, message, path, details }
  throw error;                                // conmutar por error.code (ver errors.md)
}
const page: PageResponse<Product> = await res.json();
```

## Contrato de decimales y dinero (importante)

La API **no** usa el mismo tipo JSON para dinero y para cantidades de inventario:

| Campo | Tipo JSON | Ejemplo | Notas |
| --- | --- | --- | --- |
| `product.unitPrice`, `modifierOption.priceDelta`, `sale.totalAmount`, `payment.amount`/`tenderedAmount`/`changeAmount`, `cashShift.openingAmount`/`declaredCashAmount` | **number** | `35.0` | Escala monetaria `NUMERIC(19,4)`, hasta 4 decimales. |
| `inventoryStock.quantityOnHand`, `inventoryMovement.quantityDelta`, y las cantidades en **requests** (`quantity`, `quantityDelta`) | **string** | `"100.000000"` | Cantidad de inventario, hasta 6 decimales. **Va como string** para no perder precisión. |
| `saleItem.quantity` (líneas de venta) | **integer** | `3` | Conteo entero, no decimal. |

Regla: **dinero = number**, **cantidad de inventario = string**, **cantidad de
línea de venta = entero**. Al enviar inventario, mandar el string tal cual
(`"2.500000"`), sin `parseFloat`.

## Concurrencia optimista (`expectedVersion`)

Toda mutación de un recurso versionado exige `expectedVersion` (en ventas,
`expectedSaleVersion`): la versión que el cliente **leyó**. El servidor no la
fija; solo la compara.

```json
{ "expectedVersion": 3, "name": "Nuevo nombre" }
```

Si no coincide → `409 CONCURRENT_MODIFICATION`. Patrón recomendado:
leer → mutar con la versión leída → si `409`, releer y reintentar.

## Flujo operativo de una venta

```mermaid
sequenceDiagram
    participant UI as Frontend
    participant API as Backend
    UI->>API: GET /auth/me + GET /auth/me/branches
    UI->>API: GET .../branches/{branchId}/pos/catalog/products
    UI->>API: POST .../cash-shifts/{cashShiftId}/sales   (crea venta OPEN)
    UI->>API: POST .../sales/{saleId}/items              (expectedSaleVersion)
    UI->>API: POST .../sales/{saleId}/checkout           (payments + expectedSaleVersion)
    API-->>UI: 200 (venta completada; inventario descontado) | 409
```

- Los precios y totales son **server-owned**: el cliente envía IDs, cantidades y
  `PaymentInstruction` (método + importe), nunca precios.
- El checkout es atómico: registra pagos y descuenta inventario. Un
  `409 INSUFFICIENT_INVENTORY` trae los faltantes en `details`.

## Paginación

Usar `page` (0-based) y `size` (máx 100) sobre las colecciones que lo soportan y
leer `PageResponse` (`items`, `page`, `size`, `totalElements`, `totalPages`).
Ver [`pagination.md`](pagination.md).

## Checklist de integración

- [ ] `Authorization: Bearer` en todo salvo `login`/`refresh`/`logout`.
- [ ] Un intento de `refresh` ante `401`; re-login si falla.
- [ ] Conmutar por `code` de `ApiError`, no por `message`.
- [ ] Dinero como `number`; cantidad de inventario como `string`.
- [ ] Enviar `expectedVersion` en cada mutación y manejar `409`.
- [ ] Formatear importes con `currency`/`locale` de `GET /auth/me`.
- [ ] No asumir la branch activa: descubrirla con `GET /auth/me/branches`.

Referencias: [`errors.md`](errors.md) · [`authentication.md`](authentication.md) ·
[`organization-and-branch-context.md`](organization-and-branch-context.md) ·
`api-guidelines.md`.
