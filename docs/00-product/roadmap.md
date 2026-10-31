# Roadmap conceptual

**Orientación, no contrato.** Los hitos y fechas pueden cambiar según aprendizajes.
Ningún milestone posterior al actual está garantizado en contenido ni fecha.

Release publicada: `v0.3.0` Inventory. Ciclo en desarrollo: `0.4.0-SNAPSHOT` —
**Free Local / Rust Cutover** (release técnica).

## Hitos

| Milestone | Contenido (futuro salvo indicación) | Estado |
| --- | --- | --- |
| **v0.1.x Foundation** | Estructura backend + frontend, config, seguridad base, documentación, **Organization + Branch (implementado)**, **Authentication (implementado)**, **Users administration (implementado)**, **Roles/Permissions/RBAC (implementado)**, **Branch Memberships (implementado)**, **Audit Foundation (implementado)**, **Secure Admin Bootstrap (implementado)**, **Foundation Hardening**, **estado legal propietario** | **RELEASED — `v0.1.0` (2026-10-10, release técnica privada)** |
| v0.2.x POS Core | Catálogo, Modifiers, Operational POS Catalog, Cash Registers/Shifts, Sales, Payments + Atomic Checkout, persisted cart, checkout UI, integrated E2E verified 3/3 | **RELEASED — `v0.2.0` (2026-10-12, release técnica privada)** |
| **v0.3.x Inventory** | Inventory Item Catalog, branch balances, immutable `OPENING`/`ADJUSTMENT` ledger y movement history (`V16`/`V17`); Product Recipes y ModifierOption inventory effects (`V18`); SaleItem consumption snapshots (C2); Organization negative stock policy (`STRICT`/`ALLOW_NEGATIVE`, `V20`); atomic checkout inventory consumption (`SALE_CONSUMPTION`, `V21`); Inventory administration UI (UI-A/UI-B/UI-C) y E2E integrado de Inventory (E2E-D); returns/reversal siguen diferidos | **RELEASED — `v0.3.0` (2026-10-14, release técnica privada)** |
| **v0.4.x Free Local / Rust Cutover** | Transición de backend Java/Spring Boot a Rust (monolito modular hexagonal) con SQLite local, React 19 y Tauri 2 como aplicación de escritorio. Incluye el alcance ya comprometido por Java (organización, sucursales, bootstrap, usuarios, RBAC, auditoría, catálogo, modificadores, cajas, turnos, ventas, pagos, inventario, recetas) **más** Purchasing (proveedores, órdenes y recepción) ya implementado en Rust. El corte es técnico: no incluye Control Plane, sincronización Pro, SaaS multi-tenant, Stripe ni reportería avanzada, que permanecen en hitos posteriores. Matriz de corte y decisiones en `docs/05-development/rust-v040-cut-matrix.md` y ADR-0026 | **En progreso — `0.4.0-SNAPSHOT`** |
| v0.5.x Expenses & Management | Gastos, mermas/pérdidas, auditoría | Futuro / pendiente |
| v0.6.x Reporting | Reportes de ventas, inventario y caja | Futuro / pendiente |
| v0.7.x Hardware | Shell desktop Tauri (impresora térmica, cajón, lectores) | Futuro / pendiente |
| v0.8.x Pilot | Primer cliente piloto en condiciones reales | Futuro / pendiente |
| v0.9.x Release Candidate | Congelamiento de alcance, estabilización | Futuro / pendiente |
| v1.0.0 | Primera release comercial estable | Futuro / pendiente |

## Notas

- La vertical POS reconstruye Branch → CashRegister → OPEN CashShift desde
  respuestas del backend, incluso para recuperar y cerrar turnos bajo Branches o
  Registers inactivos. La visibilidad de una Branch es discovery, no autorización;
  cada operación sigue dependiendo de su respuesta backend. Las selecciones de
  terminal y montos no se persisten. Product Browser + Persisted Cart y Checkout UI
  están implementados; el Sale detail del servidor es la autoridad del carrito y
  pricing, y las ventas nuevas requieren acción explícita. Checkout registra
  CASH/CARD y split tender mediante una operación atómica, con borrador efímero y
  previsualización exacta; el backend conserva la autoridad monetaria. La
  verificación E2E integrada real está verificada (3/3 en CI). No incluye
  procesamiento real de tarjetas, historial de pagos, tickets, inventario ni precios branch-specific.

- Los tags `v0.1.0` (Foundation), `v0.2.0` (POS Core) y `v0.3.0` (Inventory)
  existen como releases técnicas privadas; `v1.0.0` sigue siendo la primera
  release comercial estable. El ciclo en desarrollo es `0.4.0-SNAPSHOT` para el
  corte Rust Free Local.
- Alcance futuro de v1 en `docs/00-product/scope-v1.md`; el estado implementado se registra en este roadmap.
- Versiones y proceso en `docs/05-development/release-process.md`.
