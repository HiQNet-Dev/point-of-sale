# Roadmap conceptual

**Orientación, no contrato.** Los hitos y fechas pueden cambiar según aprendizajes.
Ningún milestone posterior al actual está garantizado en contenido ni fecha.

Versión en desarrollo: `0.2.0-SNAPSHOT`. Último milestone liberado: `v0.1.0` Foundation.

## Hitos

| Milestone | Contenido (futuro salvo indicación) | Estado |
| --- | --- | --- |
| **v0.1.x Foundation** | Estructura backend + frontend, config, seguridad base, documentación, **Organization + Branch (implementado)**, **Authentication (implementado)**, **Users administration (implementado)**, **Roles/Permissions/RBAC (implementado)**, **Branch Memberships (implementado)**, **Audit Foundation (implementado)**, **Secure Admin Bootstrap (implementado)**, **Foundation Hardening**, **estado legal propietario** | **RELEASED — `v0.1.0` (2026-10-10, release técnica privada)** |
| v0.2.x POS Core | Catálogo (**Category/Product implementado**), Modifiers (**implementado**), Cash Registers (**implementado**), Cash Shifts (**implementado**), Self Operational Context (`/auth/me` + `/auth/me/branches`, **implementado**), Frontend Auth/Session (**implementado**), POS Session UI (**implementado**), Sales Foundation (**implementado**), Payments + Atomic Checkout (**candidate / PR abierto**), tickets | **IN DEVELOPMENT** |
| v0.3.x Inventory | Ingredientes, recetas, existencias por sucursal | Futuro / pendiente |
| v0.4.x Purchasing | Proveedores, órdenes de compra, recepción | Futuro / pendiente |
| v0.5.x Expenses & Management | Gastos, mermas/pérdidas, auditoría | Futuro / pendiente |
| v0.6.x Reporting | Reportes de ventas, inventario y caja | Futuro / pendiente |
| v0.7.x Hardware | Shell desktop Electron (impresora térmica, cajón, lectores) | Futuro / pendiente |
| v0.8.x Pilot | Primer cliente piloto en condiciones reales | Futuro / pendiente |
| v0.9.x Release Candidate | Congelamiento de alcance, estabilización | Futuro / pendiente |
| v1.0.0 | Primera release comercial estable | Futuro / pendiente |

## Notas

- La vertical candidata de POS Session reconstruye Branch → CashRegister → OPEN
  CashShift desde respuestas del backend, incluso para recuperar y cerrar turnos
  bajo Branches o Registers inactivos. La visibilidad de una Branch es discovery,
  no autorización; cada operación sigue dependiendo de su respuesta backend.
  Las selecciones de terminal y montos no se persisten. Sales Foundation está implementada; Payments + Atomic Checkout es candidata: añade Sale COMPLETED y pagos inmutables CASH/CARD en una transacción, sin UI, tickets, CashMovements ni Inventory.

- El **tag `v0.1.0`** existe (release técnica privada de la Foundation,
  2026-10-10); `v1.0.0` sigue siendo la primera release comercial estable.
- Alcance detallado en `docs/00-product/scope-v1.md` (todo futuro).
- Versiones y proceso en `docs/05-development/release-process.md`.
