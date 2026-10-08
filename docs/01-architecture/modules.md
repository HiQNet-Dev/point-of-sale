# Módulos planeados

**Organization y Branch están implementados.** El resto de los módulos son
previstos; cada uno se marca como **no implementado** hasta que llegue su
milestone (ver `docs/00-product/roadmap.md`).

## Tabla de módulos

| Módulo | Responsabilidad | Conceptos clave | Estado |
| --- | --- | --- | --- |
| `auth` | Autenticación y sesiones | login, tokens/sesiones, perfil | **no implementado** |
| `organization` | Empresas/organizaciones | organization, scoping de catálogo | **implementado** |
| `branch` | Sucursales | branch, scoping de existencias | **implementado** |
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
| `shared` | Utilidades transversales | config, error (`ApiError`, `BusinessException`, `ErrorCodes`) | **parcial (fundación)** |

## Convención de paquetes

```text
com.hiqnet.pos.<modulo>[.domain|.application|.infrastructure|.api]
```

- Split `domain/application/infrastructure/api` **opcional**: solo se usa cuando
  el módulo lo justifica; en módulos pequeños se mantiene mínimo.
- `organization` y `branch` usan `domain` (entidades, enums, repositorios
  Spring Data), `application` (servicios y commands) y `api` (controllers y
  DTOs). **No** hay `infrastructure` propio: Spring Data genera la
  implementación de repositorios, no existe código de persistencia manual.
- Dependencia entre módulos (lectura): `branch` usa
  `OrganizationRepository` de `organization` para validar existencia y
  resolución de timezone. `organization` no conoce `branch`.
- Compartido actual: `com.hiqnet.pos.shared.config` (`SecurityConfig`,
  `CorsConfig`) y `com.hiqnet.pos.shared.error`.

## Regla de scoping (repetida a propósito)

- **Catálogo → organización:** productos, variantes e ingredientes se definen a
  nivel de organización.
- **Existencias → sucursal:** el stock nunca es global cuando hay varias
  sucursales; `inventory` siempre resuelve por branch.
- **Aislamiento de branch:** toda operación sobre una branch pasa por
  `organization_id` (`findByIdAndOrganizationId`); los ids de branch nunca se
  resuelven de forma global. Convención futura del sistema.

## Estado actual del código

Existen `com.hiqnet.pos.shared`, los módulos `organization` y `branch`
(implementados: migración Flyway `V2`, entidades, servicios, `POST/GET/PATCH`
bajo `/api/v1`, tests de integración con Testcontainers) y las clases de
arranque. La API de negocio está **deny-by-default** hasta que exista auth
(véase `docs/04-security/security-model.md`).