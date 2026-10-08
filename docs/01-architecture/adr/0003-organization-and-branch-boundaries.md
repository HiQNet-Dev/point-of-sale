# ADR-0003: Límites de Organization y Branch

## Status

Accepted

## Context

El primer módulo de negocio real necesita fijar dónde vive la configuración,
dónde es el límite operativo y qué se garantiza cuando lleguen inventario,
cajas y ventas. También hay que aclarar qué NO es esto: `organization_id` en
las tablas no implica multi-tenancy SaaS compartido.

## Decision

1. **Organization es la raíz de configuración** (raíz del agregado): moneda,
   timezone, locale y code pertenecen a la organización; ramas y (futuro)
   catálogo y usuarios cuelgan de ella.
2. **Branch es el límite operativo**: cajas, turnos, stock y dinero de ventas
   se resolverán SIEMPRE por branch; nunca globalmente.
3. **Aislamiento por organización como convención**: ninguna operación API
   resuelve una branch por id global; se exige `organization_id`
   (`findByIdAndOrganizationId`). El mismo code de branch puede existir en
   organizaciones distintas; el code de organización es único global.
4. **Sin hard delete**: la baja es `status = INACTIVE` (futuras entidades
   financieras inmutables).
5. **Multi-tenancy SaaS NO implementado**: `organization_id` escala datos
   dentro de una instalación. El despliegue sigue siendo
   `Cliente A → DB A → Organization A`. Quedan fuera: tenant resolver,
   subdomain tenancy, row-level security, schema-per-tenant, tenant JWT,
   control plane, suscripciones y licensing.

## Consequences

- Los módulos futuros (catalog, inventory, sales) deben seguir el patrón de
  aislamiento por organización/ramas desde el día uno.
- No se pueden crear "IDs mágicos" `organizationId = 1`: ids UUID generados
  en aplicación.
- Si algún día se adopta SaaS compartido, será un cambio deliberado con su
  propio ADR y migraciones; no un efecto colateral de `organization_id`.
- `branch` depende (lectura) de `organization` para validar existencia y
  heredar timezone; dependency direction única, sin ciclos.