# Inventario AS-IS del backend Java — base para la migración Rust

**Estado:** auditoría de solo lectura sobre `origin/dev@329c0984722e8339f3e3b29497960d426ac2c43c`.
Complementa la [matriz de paridad](rust-parity-baseline.md) previa y el
[paquete PRE0D-1](rust-0d-approval-package.md). **Ningún módulo está migrado a
Rust**: la columna de destino registra `NO INICIADO` salvo la fundación técnica ya
existente (`pos-core`, storage SQLite, lab). No sustituye los gates DDL13/PAR/AS
ni acredita paridad.

## 1. Método y alcance

- Lectura directa de `backend/src/main/java/com/hiqnet/pos`, sus 27 controllers,
  133 archivos `*Test.java` (861 anotaciones `@Test`), 25 migraciones Flyway y los
  consumidores de `frontend/src`.
- Conteos obtenidos por búsqueda reproducible (`find`, `grep`); no se ejecutó
  Maven, PostgreSQL ni Flyway en esta auditoría. Los resultados de CI remota de
  `329c098` constan en el paquete PRE0D-1.
- Se distingue **AS-IS** (lo que el código hace), **contrato expuesto** (lo que
  React consume) y **propuesta** (destino Rust). Una regla documentada en F4–21
  que el código no implementa **no** se marca como existente.

## 2. Matriz por módulo

| Módulo | Implementación Java verificada | Contrato expuesto | Persistencia / locks | Pruebas | Destino Rust |
| --- | --- | --- | --- | --- | --- |
| `organization` | `Organization`, `OrganizationStatus`, `InventoryNegativeStockPolicy`, `OrganizationService`; alta/listado/consulta/patch administrativo | `/organizations` (POST), `/{id}` (GET, PATCH) | `organizations`; policy de stock negativo persistida; sin lock propio | 3 archivos (org admin/API/audit rollback) | **NO INICIADO** (Incremento 1) |
| `branch` | `Branch`, `BranchStatus`, `BranchService`; CRUD parcial | `/organizations/{org}/branches` | `branches`; aislamiento por organización | 2 archivos (`BranchApiTest`, `OrganizationIsolationTest`) | **NO INICIADO** (Incremento 1) |
| `bootstrap` | `InstallationBootstrap`, `BootstrapService`; bootstrap único con token | `/bootstrap` (POST) | `installation_bootstrap`; `PESSIMISTIC_WRITE` sobre fila singleton; estado terminal | 8 archivos (concurrencia, constraints, token, disabled) | **NO INICIADO** (Incremento 1) |
| `users` | `User`, `UserStatus`, `UserService`; alta, listado, detalle, patch; email único | `/organizations/{org}/users` | `users`; unicidad de email; carrera cubierta por test | 3 archivos | **NO INICIADO** (Incremento 2) |
| `auth` | `AuthService`, `JwtTokenService`, `SelfBranchService`, refresh sessions | `/auth/login`, `/refresh`, `/logout`, `/logout-all`, `/me`, `/me/branches` | `users`, `refresh_sessions`; `SELECT ... FOR UPDATE` con orden de locks usuario→sesión | 12 archivos (JWT, revocación, concurrencia, deny) | **NO INICIADO**; transición BCrypt→Argon2id es F4–19, aún sin implementación |
| `membership` | `BranchMembership`, `BranchMembershipRole`, estados; CRUD y asignación de roles | `/organizations/{org}/branches/{b}/memberships` y `/{userId}/roles` | `branch_memberships`, `branch_membership_roles`; membresía ACTIVE exigida | 6 archivos | **NO INICIADO** (Incremento 2) |
| `rbac` | `Permission`, `Role`, `RolePermission`, `UserOrganizationRole`, `AuthorizationService` | `/permissions`, `/roles`, `/roles/{id}/permissions`, `/users/{id}/roles` | `permissions`, `roles`, `role_permissions`, `user_organization_roles`; consultas `EXISTS`, sin caché | 6 archivos (`PermissionCatalogTest`, RBAC/cross-org) | **NO INICIADO** (Incremento 2); deny-by-default debe portarse con sus queries |
| `audit` | `AuditEvent`, `AuditActorType`, `AuditQueryService`; append-only | `GET /organizations/{org}/audit-events` | `audit_events`; inmutabilidad por trigger/DB + tests | 8 archivos (rollback, actor, metadata, append-only) | **NO INICIADO** (transversal, Incremento 7) |
| `catalog` | `Category`, `Product`, `ModifierGroup`, `ModifierOption`, `ProductModifierGroup`; CRUD + catálogo operativo | `/categories`, `/products`, `/modifier-groups`, `/options`, `/products/{id}/modifier-groups`, `/pos/catalog/*` | `categories`, `products`, `modifier_groups`, `modifier_options`, `product_modifier_groups`; `PESSIMISTIC_WRITE` en repositorios | 11 archivos (incluye API operativa y constraints) | **NO INICIADO** (Incremento 3) |
| `cashregister` | `CashRegister`, `CashRegisterStatus`, `CashRegisterService` | `/cash-registers` (POST/GET/GET id/PATCH) | `cash_registers`; lectura permitida con sucursal inactiva vía `hasBranchPermissionIgnoringBranchStatus` | 7 archivos | **NO INICIADO** (Incremento 4) |
| `cashshift` | `CashShift`, `CashShiftStatus`, `CashShiftService`; apertura/cierre | `/cash-shifts`, `/{id}`, `/{id}/close` | `cash_shifts`; `PESSIMISTIC_WRITE`; un turno OPEN por caja | 8 archivos (recuperación de sucursal inactiva incluida) | **NO INICIADO** (Incremento 4) |
| `sales` | `Sale`, `SaleItem`, `SaleItemModifier`, snapshots de consumo; checkout atómico | `/cash-registers/{r}/cash-shifts/{s}/sales` + checkout/payments/items/cancel | `sales`, `sale_items`, `sale_item_modifiers`, `payments`, `sale_item_inventory_components`; `@Transactional`; versión optimista | 9 archivos (concurrencia, idempotencia de checkout, snapshots, constraints) | **NO INICIADO** (Incremento 4) |
| `payment` | `Payment`, `PaymentMethod`; **no** existe controller propio, se crea dentro del checkout | `GET /sales/{id}/payments` | `payments`; sin escritura independiente | Cubierto por `sales` (`PaymentCheckout*`, `PaymentDbConstraintsTest`) | **NO INICIADO** (Incremento 4) |
| `inventory` (artículos) | `InventoryItem`, `InventoryBaseUnit`, `InventoryItemStatus`; guard de unidad base | `/inventory-items` | `inventory_items`; `PESSIMISTIC_WRITE`; lock monotónico de unidad base | 16 archivos en el árbol `inventory`, varios de unidad/constraints | **NO INICIADO** (Incremento 5) |
| `inventory.stock` | `BranchInventory`, `InventoryMovement`, `InventoryMovementType`; apertura/ajuste/consumo | `/inventory-items/{id}/stock`, `/movements`, `/opening-stock`, `/adjustments` | `branch_inventory`, `inventory_movements`; ledger append-only; `PESSIMISTIC_WRITE` | `InventoryMovementAppendOnlyTest`, `InventoryQuantityTest`, stock/concurrencia/rollback | **NO INICIADO** (Incremento 5) |
| `inventory.recipe` / `modifier` | `ProductInventoryRecipe`, componentes, efectos de modificadores | `/products/{id}/inventory-recipe`, `/modifier-options/{id}/inventory-effects` | `product_inventory_recipes`, componentes, configs/efectos | `InventoryRecipe*`, `ModifierInventoryEffectsApiTest` | **NO INICIADO** (Incremento 5) |
| `purchasing` | `Supplier`, `PurchaseOrder`, `PurchaseOrderLine`, `PurchaseReceipt`, `PurchaseReceiptLine`, servicios de emisión/cancelación/recepción | 4 controllers: suppliers, purchase-orders, líneas, receipts, receipt-status | `suppliers`, `purchase_orders`, `purchase_order_lines`, `purchase_receipts`, `purchase_receipt_lines`; idempotencia por `(organization, purchase_order, idempotency_key)` + fingerprint SHA-256 | 22 archivos (transiciones, concurrencia, rollback, idempotencia, constraints) | **NO INICIADO** (Incremento 6); depende de D01–D07 |
| `shared` | `ApiError`/`ErrorCodes`, `ConstraintTranslator`, `PageResponse`, `PatchField`, `SecurityConfig`, CORS, config de auth/OpenAPI | Forma de error y paginación comunes; `OpenAPI` en `/v3/api-docs` | No aplica | 5 archivos + tests de OpenAPI/CORS/config | **NO INICIADO** (transversal) |

## 3. Evidencia relevante para la paridad

### 3.1 Autorización

`AuthorizationService` documenta y aplica dos scopes: organización y sucursal. Un
rol de sucursal **no** se promueve a organización. Las lecturas de caja/turno usan
variantes que ignoran el estado de la sucursal para permitir recuperación; las
mutaciones exigen sucursal ACTIVE. Cualquier puerto Rust debe conservar esa
distinción: simplificarla a un único `hasPermission` cambiaría comportamiento.

### 3.2 Costo de compra

`V23` define `purchase_order_lines.unit_cost NUMERIC(19,4) NULL CHECK (...>=0)` y
el comentario SQL declara que **no representa valoración de inventario ni cuentas
por pagar**. `PurchaseOrderLine` y los comandos Java permiten `null` explícito y
`PATCH` distingue “campo ausente” de “null para limpiar”. La recepción
(`PurchaseReceiptLine`) guarda **solo cantidad recibida**: no existe costo efectivo
propio en el origen. Esto confirma la frontera D03/D04: el valor de entrada no se
puede inferir del catálogo ni del costo pactado, y el import debe declarar la
ausencia.

### 3.3 Idempotencia y atomicidad

La recepción es inmutable, usa clave de idempotencia única por organización/orden y
fingerprint canónico para distinguir reintento idéntico de conflicto. El checkout y
la recepción son transaccionales y tienen pruebas de concurrencia y rollback. La
paridad no puede reducirse a comparar una respuesta 200: deben compararse venta,
pagos, ledger y snapshots.

### 3.4 Ledger

`inventory_movements` es append-only y `V25` añade `PURCHASE_RECEIPT` con FK
compuesta a la línea de recepción y unicidad por línea. Los movimientos manuales y
de venta mantienen `purchase_receipt_line_id = NULL`.

## 4. Diferencias y deudas observadas (no corregidas aquí)

| Hallazgo | Evidencia | Implicación |
| --- | --- | --- |
| React tipa solo `OPENING`, `ADJUSTMENT`, `SALE_CONSUMPTION` | `frontend/src/features/inventory-stock/api/inventoryStockApi.ts:6` | El backend/V25 ya emite `PURCHASE_RECEIPT`; la UI no lo muestra. Es una brecha de contrato existente, no un defecto de la migración. No se modificó React. |
| `payment` no tiene controller | No existe `payment/api/*Controller.java`; el alta ocurre en checkout | No crear un endpoint “payment” nuevo por simetría; el contrato real es el de ventas. |
| Recepción sin costo efectivo | `PurchaseReceiptLine` solo tiene `quantityReceived` | D03/D04 no pueden cerrarse con un `JOIN` al catálogo. |
| Costo pactado nullable incluso en órdenes emitidas/recibidas | `V23` + `PurchaseOrderLine` | Confirma PHY-PAR-03; NULL ≠ 0. |
| Diferencias Java/Rust no existen todavía | No hay módulos Rust de dominio equivalentes | “Paridad” sigue siendo objetivo, no resultado. |

## 5. Orden de migración propuesto y dependencias

1. **Incremento 1:** organización, sucursal y bootstrap (sin usuarios reales).
2. **Incremento 2:** users/auth/membership/RBAC, con AUTH-MIG y F4–19.
3. **Incremento 3:** catálogo y contrato operativo React.
4. **Incremento 4:** caja, turnos, ventas, pagos.
5. **Incremento 5:** inventario, ledger, recetas y costeo aprobado.
6. **Incremento 6:** proveedores, compras y recepciones (requiere D01–D07).
7. **Incremento 7:** auditoría, errores, paginación, OpenAPI y exportaciones.

Cada incremento debe incluir pruebas de autorización cruzada, atomicidad,
idempotencia y regresión de contrato. La mera compilación del workspace Rust no
acredita ninguno de estos pasos.
