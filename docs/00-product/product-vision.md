# Visión de producto — HiQNet POS

**Estado:** Inventory publicado (`v0.3.0`); candidato `v0.4.0` (Purchasing) en preparación. Expenses, Reports, returns/reversal y hardware siguen pendientes.

## Qué es

HiQNet POS es un sistema de punto de venta **modular** y **comercial** de HiQNet,
diseñado para pequeños negocios de alimentos y retail ligero:

- Cafeterías
- Juguerías
- Restaurantes pequeños
- Retail ligero

Es un producto **reutilizable**: una misma base de código se despliega como
instalación aislada por cliente (ver `docs/01-architecture/architecture.md`).

## Segmentos y evolución

**Foco inicial:** un negocio simple con una sucursal, una caja y venta de mostrador.

**Crecimiento previsto (pendiente):**

- Inventario por sucursal, ingredientes y recetas
- Compras y proveedores
- Reportes y administración
- Hardware POS (impresora térmica, cajón de dinero, lectores)
- Licenciamiento y suscripciones

## Referencia legada

Existió un proyecto legado de referencia que funciona como **solo referencial
funcional**: de él pueden consultarse lecciones de dominio y decisiones de
producto. **No se copia** su arquitectura, ni su modelo de datos, ni sus
migraciones, ni su código. HiQNet POS parte de cero con su propia arquitectura.

## Principios de producto

1. **Simplicidad** — la solución más simple que resuelva el caso real.
2. **Modularidad** — módulos con límites claros; crecer por módulos, no por acoplamiento.
3. **Seguridad** — deny by default, autorización server-side, least privilege.
4. **Testabilidad** — reglas críticas cubiertas por pruebas; PostgreSQL real vía Testcontainers.
5. **Observable** — salud y métricas expuestas con exposición mínima.
6. **Cero sobreingeniería** — no se introducen tecnologías sin un requisito real
   (ver ADR-0001 y ADR-0002).

## Licencia

**Software propietario de HiQNet.**

- **Copyright:** © 2026 HiQNet. All rights reserved.
- **Open-source license:** ninguna; no se concede ninguna licencia general de
  reutilización del repositorio ni del código.
- **Repositorio:** privado.
- **Uso por clientes/terceros:** únicamente mediante acuerdos comerciales
  escritos separados con HiQNet.
- Las releases técnicas privadas `v0.1.0` (Foundation), `v0.2.0` (POS Core) y
  `v0.3.0` (Inventory), así como el candidato `v0.4.0` (Purchasing), no
  sustituyen los acuerdos comerciales de clientes.

Detalle: `LICENSE` y `docs/05-development/legal-status.md`.
