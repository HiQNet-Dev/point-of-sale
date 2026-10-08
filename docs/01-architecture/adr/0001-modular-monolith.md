# ADR-0001: Modular Monolith sobre microservicios

## Status

Accepted

## Context

HiQNet POS es un producto comercial de punto de venta para pequeños negocios,
con equipo pequeño y despliegues **aislados por cliente** (una instalación por
cliente, no multi-tenant SaaS). Las operaciones críticas —venta, pagos,
inventario— son **transaccionales y fuertemente consistentes**: una venta debe
descontar stock y registrar pagos de forma atómica.

Se evaluó la opción de microservicios frente a un monolito modular.

## Decision

Adoptar un **monolito modular**: un único desplegable Spring Boot que contiene
los módulos de negocio con límites explícitos (ver `docs/01-architecture/modules.md`
y reglas de dependencia en `architecture.md`).

Justificación:

1. **Equipo pequeño.** Microservicios añaden operación (red, despliegues
   coordinados, contratos versionados) sin aportar velocidad a un equipo pequeño.
2. **Un solo desplegable.** Simplifica actualización en instalaciones aisladas
   por cliente: copiar/actualizar un servicio, no una malla.
3. **Transacciones locales.** Venta + pagos + inventario se resuelven con
   transacciones ACID en una sola base PostgreSQL, sin sagas ni eventos
   compensatorios.
4. **Escalabilidad modesta.** El requisito real es de nivel sucursal/caja; un
   monolito bien hecho escala horizontalmente si hiciera falta.

Se rechazan los microservicios **hasta que exista un driver real** (requisito de
escalamiento independiente o independencia organizativa de equipos), no por
anticipación.

## Consequences

- Los límites de módulo se protegen por convención y revisión; un monolito
  permite que se erosionen si no se vigilan (riesgo aceptado, mitigado con
  reglas de dependencia y tests).
- Despliegue único: liberación conjunta de módulos en una versión.
- Escalar un módulo con mucha carga exigiría extraerlo en el futuro (decisión
  posterior, no ahora).
- Sin coste de infraestructura de mensajería ni orquestación en esta etapa.
