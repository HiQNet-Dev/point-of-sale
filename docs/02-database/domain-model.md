# Modelo de dominio conceptual

**Documento vivo.** `Organization`, `Branch`, `User` (identidad) y
`RefreshSession` ya están implementados (migraciones `V1`–`V3`); el resto de
las entidades descritas aquí son futuras.

Versión actual: `0.1.0-SNAPSHOT`.

## Jerarquía principal

```text
Organization (organización / empresa)  ✔ implementado
 ├── User (usuario)                    ✔ implementado (identidad; RBAC futuro)
 │    └── RefreshSession (familia de refresh tokens)  ✔ implementado
 └── Branch (sucursal)                 ✔ implementado
      ├── CashRegister (caja)  → Shift (turno) → Sale (venta) → Payment (pago)
      ├── Stock (existencias)  ← descuenta por venta, compra y mermas
      └── Inventory (ámbito de inventario de la sucursal)
```

## Entidades previstas (futuro)

| Entidad | Ámbito | Notas |
| --- | --- | --- |
| Organization | raíz | **implementada** — ver `docs/02-database/organization-branch-model.md` |
| Branch | organización | **implementada** — ver `docs/02-database/organization-branch-model.md` |
| User | organización | **implementada (identidad)** — email normalizado único por organización, `password_hash` BCrypt, intentos/lockout; RBAC futuro |
| RefreshSession | usuario | **implementada** — refresh opaco rotativo; solo SHA-256 persistido; familia 30d absolutos (`docs/04-security/authentication.md`) |
| Product / Variant | **organización** | Catálogo: tamaños, sabores, precios |
| Ingredient | **organización** | Insumos del catálogo |
| Recipe | **organización** | Producto → ingredientes + cantidades |
| Stock | **sucursal** | Existencias por ingrediente/producto en cada branch |
| CashRegister / Shift | sucursal | Cajas y arqueos |
| Sale / SaleLine / Payment | sucursal | Dinero siempre en `BigDecimal` |
| Supplier / PurchaseOrder | organización | Compras (futuro) |
| Expense | sucursal u organización | Gastos (futuro) |

## Regla clave (repetida)

**El inventario NO es global cuando hay varias sucursales: las existencias
pertenecen a la sucursal.** El catálogo (productos, ingredientes, variantes)
pertenece a la organización; el stock se consulta y modifica siempre por
`branch`. Nada de `branchId` fijo ni IDs mágicos (ver
`docs/05-development/coding-guidelines.md`).

**Aislamiento:** ningún recurso de una rama se resuelve por id genérico; toda
operación pasa por `organization_id` (`findByIdAndOrganizationId`).

## Convenciones de dinero y tiempo

- **Dinero:** `BigDecimal` siempre. Nunca `double`/`float`. Escala monetaria
  definida por columna (p. ej. `numeric(19,4)` a decidir en el ER final).
- **Moneda:** `currency` de la organización en ISO 4217 (validado con Java
  `Currency`); `char(3) → varchar(3)` + CHECK en DDL.
- **Timestamps:** `created_at` / `updated_at` en **UTC** (`TIMESTAMPTZ`,
  `Instant` en JPA) en todas las entidades; zona horaria del negocio
  configurable por organización (con herencia a branch).
- **Auditoría:** quién/cuándo/qué; operaciones de dinero inmutables
  (sin hard delete) — ver `docs/04-security/security-model.md`.

## Relaciones

- Organization 1—N Branch ✔; Organization 1—N User ✔; Organization 1—N Product/Ingredient (futuro).
- User 1—N RefreshSession ✔ (sesiones de refresh por familia).
- Branch 1—N CashRegister, Branch 1—N Stock (futuro).
- Product 1—N Variant; Recipe N—M Ingredient (con cantidades) (futuro).
- Sale 1—N Payment; Sale 1—N SaleLine; Sale N—1 Branch/CashRegister (futuro).
