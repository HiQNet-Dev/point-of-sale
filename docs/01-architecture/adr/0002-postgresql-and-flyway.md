# ADR-0002: PostgreSQL y Flyway como base del esquema

## Status

Accepted

## Context

El producto necesita persistencia relacional con integridad fuerte y
transacciones (operaciones de dinero e inventario). El esquema evolucionará
durante toda la vida del producto, con despliegues aislados por cliente donde
las migraciones deben ejecutarse de forma controlada y reproducible.

Se evaluó qué base de datos usar y qué mecanismo —y único— gobierna el esquema.

## Decision

1. **PostgreSQL 17** como único motor de base de datos.
2. **Flyway** (gestionado por Spring Boot) como **único** mecanismo de
   evolución de esquema.
3. `spring.jpa.hibernate.ddl-auto: validate` en todos los entornos.
   **Nunca `update`** (ni `create`/`drop`) como fuente de esquema.
4. Migraciones **versionadas**, **inmutables una vez aplicadas** en cualquier
   entorno, y **aditivas preferidas** sobre cambios destructivos.
5. **Sin sustitutos H2** ni otros bancos de prueba: las pruebas de integración
   usan **Testcontainers con PostgreSQL real**.

Justificación:

- **PostgreSQL:** integridad referencial y transacciones, madurez, disponibilidad
  en despliegues, y `JSONB` como opción futura sin salir del motor.
- **Flyway único:** un solo camino de evolución elimina ambigüedades sobre quién
  creó qué columna; las migraciones viajan con el código y se aplican al arrancar.
- **`validate`:** si el código y el esquema divergen, la aplicación **falla
  temprano** en lugar de alterar el esquema silenciosamente.

## Consequences

- Cualquier cambio de esquema requiere **una nueva migración + revisión en PR**;
  editar migraciones ya aplicadas está prohibido (se corrige con una nueva).
- Las pruebas dependen de Docker (Testcontainers); es un requisito de entorno.
- No hay modo "rápido" con H2: menos sorpresas de compatibilidad, a costa de
  un poco más de tiempo de prueba.
- Rollbacks de migración no son automáticos: se resuelven con migraciones
  compensatorias aditivas cuando hace falta.
