# Roadmap conceptual

**Orientación, no contrato.** Los hitos y fechas pueden cambiar según aprendizajes.
Ningún milestone posterior al actual está garantizado en contenido ni fecha.

Versión en desarrollo: `0.2.0-SNAPSHOT`. Último milestone liberado: `v0.1.0` Foundation.

## Hitos

| Milestone | Contenido (futuro salvo indicación) | Estado |
| --- | --- | --- |
| **v0.1.x Foundation** | Estructura backend + frontend, config, seguridad base, documentación, **Organization + Branch (implementado)**, **Authentication (implementado)**, **Users administration (implementado)**, **Roles/Permissions/RBAC (implementado)**, **Branch Memberships (implementado)**, **Audit Foundation (implementado)**, **Secure Admin Bootstrap (implementado)**, **Foundation Hardening**, **estado legal propietario** | **RELEASED — `v0.1.0` (2026-10-10, release técnica privada)** |
| v0.2.x POS Core | Catálogo (**Category/Product implementado**), Modifiers (**implementado**), Operational POS Catalog (**candidate / PR abierto**), Cash Registers (**implementado**), Cash Shifts (**implementado**), Self Operational Context (`/auth/me` + `/auth/me/branches`, **implementado**), Frontend Auth/Session (**implementado**), POS Session UI (**implementado**), Sales Foundation (**implementado**), Payments + Atomic Checkout (**implementado**), tickets | **IN DEVELOPMENT** |
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
  Las selecciones de terminal y montos no se persisten. Sales Foundation y Payments
  + Atomic Checkout están implementados. Operational POS Catalog es candidato:
  `pos.catalog.read` habilita Category/Product/configuración de modifiers bajo
  Branch ACTIVE sin dar permisos administrativos, sin UI, inventario, tickets ni
  precios branch-specific.

- El **tag `v0.1.0`** existe (release técnica privada de la Foundation,
  2026-10-10); `v1.0.0` sigue siendo la primera release comercial estable.
- Alcance detallado en `docs/00-product/scope-v1.md` (todo futuro).
- Versiones y proceso en `docs/05-development/release-process.md`.
