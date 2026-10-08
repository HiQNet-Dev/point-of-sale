# Guía de código

Reglas numeradas aplicables desde la fundación. Cada regla incluye el porqué.

1. **No hard delete de operaciones financieras.**
   Ventas, pagos y movimientos de dinero se dan de baja lógicamente; borrarlos
   destruye auditoría y cuadre contable.

2. **Dinero siempre con `BigDecimal`; nunca `double`/`float`.**
   Los flotantes pierden centavos; el dinero no admite error de redondeo.

3. **Operaciones monetarias transaccionales.**
   Venta + pagos + descuento de stock se aplican atómicamente; un fallo parcial
   deja el negocio inconsistente.

4. **Esquema solo con Flyway; jamás `ddl-auto=update`.**
   `update` altera el esquema sin revisión ni registro; Flyway da historial,
   review y reproducibilidad (ver ADR-0002).

5. **No secretos en Git.**
   Todo lo sensible va a variables de entorno por ambiente; Git es público para
   siempre.

6. **No IDs mágicos (sin `branchId=1` ni equivalentes).**
   Un ID fijo rompe multi-sucursal y contamina datos entre entidades; el
   contexto siempre llega explícito (organization/branch).

7. **Sin lógica de negocio en controllers.**
   Controllers traducen HTTP; la regla vive en application/domain para poder
   probarla sin HTTP y reutilizarla.

8. **Sin lógica crítica en React.**
   El servidor es la fuente de verdad; lo del frontend es UX. Un cliente
   manipulado no puede autorizar nada.

9. **Persistencia en UTC; zona horaria del negocio configurable.**
   UTC evita ambigüedad entre entornos y DST; el negocio configura su zona
   por encima.

10. **Validación server-side siempre.**
    La validación del cliente se evita con DevTools; solo el servidor protege
    integridad y seguridad.

11. **Tests para las reglas críticas.**
    Dinero, inventario y permisos necesitan regresión automatizada;
    SQL-dependent usa **Testcontainers con PostgreSQL real** — **sin H2 como
    sustituto** (H2 no es PostgreSQL y oculta incompatibilidades).

12. **No ignorar tests que fallan.**
    Un test rojo ignorado anula el valor de toda la suite.

13. **Commits pequeños + Conventional Commits.**
    Historial legible y changelog automatizable (ver `git-workflow.md`).

Validación mínima antes de dar por terminada una tarea:

```bash
cd backend && ./mvnw clean verify     # compila + pruebas (Testcontainers)
cd frontend && pnpm lint && pnpm typecheck && pnpm test && pnpm build
```
