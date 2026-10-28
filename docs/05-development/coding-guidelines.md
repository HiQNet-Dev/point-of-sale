# Guía de código

Reglas numeradas aplicables desde la fundación. Cada regla incluye el porqué.

1. **No hard delete de operaciones financieras.**
   Ventas, pagos y movimientos de dinero se dan de baja lógicamente; borrarlos
   destruye auditoría y cuadre contable.

2. **Dinero exacto: Java `BigDecimal`; Rust `MoneyMinor` y operandos decimales exactos.**
   Nunca `double`/`float`, `f32`/`f64` ni `REAL` autoritativo. MoneyMinor usa
   unidades menores, moneda explícita y aritmética checked. Redondeo versionado,
   no implícito; no alterar cantidades ni dinero históricos para hacerlos caber.

3. **Operaciones monetarias transaccionales.**
   Venta + pagos + descuento de stock se aplican atómicamente; un fallo parcial
   deja el negocio inconsistente.

4. **Esquema versionado por su autoridad; jamás `ddl-auto=update`.**
   Java/PostgreSQL AS-IS conserva Flyway (ADR-0002) y V1–V25 inmutables.
   F4–18/F4–21 y ADR-0020 establecen SQLx para Rust con roots Local SQLite,
   Central PostgreSQL y Control PostgreSQL independientes. No convertir
   historiales ni reutilizar V1–V25. Los gates de migración deben demostrar
   clean/upgrade/checksum/failure/restore antes de aprobar su alcance.

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

En Rust, documentar APIs públicas con rustdoc en español y comentar decisiones,
seguridad, invariantes y transacciones no obvias. Sin `unwrap`, `expect` ni
`panic!` injustificados en producción. `pos-core` no importa infraestructura.
La [guía Rust](../../backend-rust/README.md) documenta los comandos Cargo y los
gates todavía bloqueados; no declarar paridad por pasar fixtures técnicas.

Validación mínima Java/React antes de dar por terminada una tarea que los afecte:

```bash
cd backend && ./mvnw clean verify     # compila + pruebas (Testcontainers)
cd frontend && pnpm lint && pnpm typecheck && pnpm test && pnpm build
```
