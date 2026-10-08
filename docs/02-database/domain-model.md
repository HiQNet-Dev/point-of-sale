# Modelo de dominio conceptual

**Documento conceptual, no definitivo.** El ER final se definirá en una tarea
posterior cuando los módulos correspondientes se implementen. **Todas las
entidades descritas aquí son futuras / no implementadas:** hoy no existe
ninguna tabla de negocio (solo la fundación; Flyway sin migraciones de dominio).

Versión actual: `0.1.0-SNAPSHOT`.

## Jerarquía principal

```text
Organization (organización / empresa)
 └── Branch (sucursal)
      ├── CashRegister (caja)  → Shift (turno) → Sale (venta) → Payment (pago)
      ├── Stock (existencias)  ← descuenta por venta, compra y mermas
      └── Inventory (ámbito de inventario de la sucursal)
```

## Entidades previstas (futuro)

| Entidad | Ámbito | Notas |
| --- | --- | --- |
| Organization | raíz | Agrupa sucursales, usuarios y catálogo |
| Branch | organización | Dueña de existencias y cajas |
| User | organización | Con roles/permisos (RBAC futuro) |
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

## Convenciones de dinero y tiempo

- **Dinero:** `BigDecimal` siempre. Nunca `double`/`float`. Escala monetaria
  definida por columna (p. ej. `numeric(19,4)` a decidir en el ER final).
- **Moneda:** considerar campo de moneda por organización desde el diseño
  (futuro; hoy sin implementar).
- **Timestamps:** `created_at` / `updated_at` en **UTC** en todas las entidades;
  zona horaria del negocio configurable por encima (futuro).
- **Auditoría:** quién/cuándo/qué; operaciones de dinero inmutables
  (sin hard delete) — ver `docs/04-security/security-model.md`.

## Relaciones futuras

- Organization 1—N Branch; Organization 1—N User; Organization 1—N Product/Ingredient.
- Branch 1—N CashRegister, Branch 1—N Stock.
- Product 1—N Variant; Recipe N—M Ingredient (con cantidades).
- Sale 1—N Payment; Sale 1—N SaleLine; Sale N—1 Branch/CashRegister.

Todo lo anterior está **pendiente / no implementado**; el ER final puede
diferir y deberá documentarse en esta misma carpeta.
