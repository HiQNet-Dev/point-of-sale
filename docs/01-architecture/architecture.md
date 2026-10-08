# Arquitectura — HiQNet POS

**Estado:** fundación. La API no expone todavía funcionalidad de negocio.

## Forma general: Modular Monolith (ver ADR-0001)

Un único desplegable Spring Boot + una SPA React. Sin microservicios.

```text
frontend/   React SPA (estática en producción)
backend/    Spring Boot 4.1.1 — Java 21, Maven wrapper 3.9.16
            REST + JSON, Spring Security, Spring Data JPA
            PostgreSQL 17 + Flyway (gestionado por Spring Boot)
            Testcontainers para integración
```

## Capas por módulo

Cada módulo puede organizarse en `api / application / domain / infrastructure`.
**No es ceremonial:** en módulos pequeños se usa la estructura mínima necesaria.
La separación existe para mantener límites, no para acumular paquetes vacíos.

## Fronteras de módulos y reglas de dependencia

- Los módulos **no se importan entre sí** directamente; colaboran a través de
  contratos compartidos (`shared` / api contracts).
- **Scoping obligatorio:** catálogo (productos, ingredientes, variantes) pertenece
  a la **organización**; las **existencias pertenecen a la sucursal**.
- Reglas de seguridad server-side en `docs/04-security/security-model.md`.
- Estructura prevista de módulos: `docs/01-architecture/modules.md` (no implementada).

## API

Convenciones resumidas en `docs/03-api/api-guidelines.md`:

- Base path `/api/v1`, JSON, `camelCase`.
- Errores con contrato estable `ApiError` (`timestamp, status, code, message, path, details`).
- Estado actual: sin endpoints de negocio; solo `/actuator/health` y `/error` permitidos.

## Persistencia

- **Flyway es el único mecanismo de evolución de esquema** (ver ADR-0002).
- `ddl-auto: validate` — jamás `update` en ningún entorno.
- Transacciones en operaciones de dinero; dinero en `BigDecimal`.
- Persistencia en **UTC**; zona horaria del negocio configurable (futuro).

## Frontera Electron (futuro, no construido)

Shell de escritorio para cajero: impresora térmica, cajón de dinero, lectores.
Se comunica con la API por puente local/HTTP. **Sin lógica de negocio en
Electron.** Detalle en v0.7.x del roadmap.

## HiQNet Control Plane (futuro, no construido)

Modelo previsto: **instalaciones aisladas por cliente** en lugar de
multi-tenancy prematuro. Un futuro "Control Plane" administraría esas
instalaciones de forma separada (alta, licencias, actualizaciones). No existe
hoy ningún código ni diseño de implementación.

## Despliegue (futuro)

Un servicio backend + frontend estático detrás de reverse proxy.
Documentación de despliegue pendiente en `deploy/`. Sin orquestación compleja.

## Qué NO se usa (y por qué)

| No usado | Motivo |
| --- | --- |
| Kafka / RabbitMQ | Sin requisito de mensajería asíncrona |
| Redis | Sin caché ni sesiones distribuidas requeridas |
| Microservicios | Ver ADR-0001 |
| CQRS / Event sourcing | Complejidad sin beneficio actual |
| GraphQL | REST cubre el caso |
| WebSockets | Ningún caso en tiempo real todavía (futuro: posible módulo) |
| Multi-tenancy SaaS | Modelo de instalaciones aisladas (futuro: Control Plane) |
