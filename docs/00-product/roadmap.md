# Roadmap conceptual

**Orientación, no contrato.** Los hitos y fechas pueden cambiar según aprendizajes.
Ningún milestone posterior al actual está garantizado en contenido ni fecha.

Versión en desarrollo: `0.3.0-SNAPSHOT`. Último milestone liberado: `v0.2.0` POS Core.

## Hitos

| Milestone | Contenido (futuro salvo indicación) | Estado |
| --- | --- | --- |
| **v0.1.x Foundation** | Estructura backend + frontend, config, seguridad base, documentación, **Organization + Branch (implementado)**, **Authentication (implementado)**, **Users administration (implementado)**, **Roles/Permissions/RBAC (implementado)**, **Branch Memberships (implementado)**, **Audit Foundation (implementado)**, **Secure Admin Bootstrap (implementado)**, **Foundation Hardening**, **estado legal propietario** | **RELEASED — `v0.1.0` (2026-10-10, release técnica privada)** |
| v0.2.x POS Core | Catálogo, Modifiers, Operational POS Catalog, Cash Registers/Shifts, Sales, Payments + Atomic Checkout, persisted cart, checkout UI, integrated E2E verified 3/3 | **RELEASED — `v0.2.0` (2026-10-12, release técnica privada)** |
| v0.3.x Inventory | Inventory Item Catalog, Branch Balance, immutable Ledger, Opening Stock, Manual Adjustments, Product Recipes and Modifier Inventory Effects implemented; Inventory Item and Branch Stock administration UI implemented (UI-A); Product Recipe and Modifier Effect administration UI implemented (UI-B); SaleItem Consumption Snapshot is candidate in PR C2; Organization negative stock policy (`V20`) is candidate in PR C3a; Atomic Checkout Stock Deduction from snapshots (`V21`) is candidate in PR C3b; negative-stock policy UI (UI-C), integrated inventory E2E (E2E-D), the v0.3 release audit and returns/reversal remain deferred | **IN DEVELOPMENT** |
| v0.4.x Purchasing | Proveedores, órdenes de compra, recepción | Futuro / pendiente |
| v0.5.x Expenses & Management | Gastos, mermas/pérdidas, auditoría | Futuro / pendiente |
| v0.6.x Reporting | Reportes de ventas, inventario y caja | Futuro / pendiente |
| v0.7.x Hardware | Shell desktop Electron (impresora térmica, cajón, lectores) | Futuro / pendiente |
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

- El **tag `v0.2.0`** existe (release técnica privada de POS Core,
  2026-10-12); `v1.0.0` sigue siendo la primera release comercial estable. La
  línea principal de desarrollo ya inició v0.3 Inventory.
- Alcance detallado en `docs/00-product/scope-v1.md` (todo futuro).
- Versiones y proceso en `docs/05-development/release-process.md`.
