# Módulos planeados

**Ningún módulo de negocio está implementado.** Esta es la estructura prevista;
cada módulo se marca como **no implementado** y llegará en milestones posteriores
(ver `docs/00-product/roadmap.md`).

## Tabla de módulos

| Módulo | Responsabilidad | Conceptos clave (futuros) | Estado |
| --- | --- | --- | --- |
| `auth` | Autenticación y sesiones | login, tokens/sesiones, perfil | **no implementado** |
| `organization` | Empresas/organizaciones | organization, scoping de catálogo | **no implementado** |
| `branch` | Sucursales | branch, scoping de existencias | **no implementado** |
| `users` | Usuarios y permisos | users, roles, RBAC | **no implementado** |
| `catalog` | Catálogo comercial | products, variants, precios, ingredientes | **no implementado** |
| `sales` | Venta y tickets | sales, tickets, líneas, totales | **no implementado** |
| `payments` | Pagos de la venta | methods, splits, vuelto | **no implementado** |
| `cash` | Cajas y turnos | cash registers, shifts, arqueos | **no implementado** |
| `inventory` | Existencias e insumos | stock por branch, ingredients, recipes, mermas | **no implementado** |
| `purchasing` | Compras | suppliers, órdenes, recepción | **no implementado** |
| `expenses` | Gastos del negocio | expenses, categorías | **no implementado** |
| `reporting` | Reportes | ventas, inventario, caja | **no implementado** |
| `audit` | Auditoría | quién/cuándo/qué, inmutabilidad en dinero | **no implementado** |
| `shared` | Utilidades transversales | config, error (`ApiError`) | **parcial (fundación actual)** |

## Convención de paquetes

```text
com.hiqnet.pos.<modulo>[.domain|.application|.infrastructure|.api]
```

- Split `domain/application/infrastructure/api` **opcional**: solo se usa cuando
  el módulo lo justifica; en módulos pequeños se mantiene mínimo.
- Compartido actual: `com.hiqnet.pos.shared.config` (`SecurityConfig`,
  `CorsConfig`) y `com.hiqnet.pos.shared.error` (`ApiError`,
  `GlobalExceptionHandler`, `ApiSecurityErrorHandler`).

## Regla de scoping (repetida a propósito)

- **Catálogo → organización:** productos, variantes e ingredientes se definen a
  nivel de organización.
- **Existencias → sucursal:** el stock nunca es global cuando hay varias
  sucursales; `inventory` siempre resuelve por branch.

## Estado actual del código

El código contiene únicamente `com.hiqnet.pos.shared` (config/error) más la
clase de arranque `PosApplication`. **El resto de los módulos llega en tareas
posteriores.** Ninguna tabla de negocio existe en Flyway todavía.
