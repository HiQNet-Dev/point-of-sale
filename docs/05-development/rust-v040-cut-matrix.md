# Matriz de corte v0.4.0 — HiQNet POS Free Local (Rust)

**Estado a 2026-10-31, tras fusionar #88 y preparar #89.**

Estados admitidos por columna: `IMPLEMENTED`, `TESTED`, `INTEGRATED`, `BLOCKED`,
`DEFERRED`. No se usan porcentajes.

- `IMPLEMENTED`: existe código Rust en dominio/aplicación/persistencia.
- `TESTED`: hay pruebas automatizadas que lo ejercitan sobre SQLite real.
- `INTEGRATED`: el flujo atraviesa la frontera real (IPC/UI o arnés integral).
- `BLOCKED`: existe trabajo pero falta una dependencia concreta para activarlo.
- `DEFERRED`: fuera del alcance comprometido de v0.4.0.

Autoridad de decisión: esta matriz y el
[tablero de migración](rust-migration-board.md). La retirada de Java exige que
ninguna fila comprometida quede en `BLOCKED`.

## Matriz

| Módulo | Java AS-IS | Rust | SQLite | IPC | UI | E2E | Decisión |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Organización | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta IPC/UI comercial |
| Sucursales | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; lectura solo en laboratorio |
| Bootstrap administrativo | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; autorización nativa lista |
| Provisioning (token/CLI) | IMPLEMENTED (secreto) | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta UI de instalación |
| Usuarios | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta login comercial |
| Autenticación / sesión | IMPLEMENTED | BLOCKED | BLOCKED | BLOCKED | BLOCKED | DEFERRED | **Bloqueo principal**: falta caso de uso de login e IPC |
| RBAC / memberships | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; resolución probada |
| Auditoría | IMPLEMENTED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | DEFERRED | Mantener; no se expone UI en v0.4.0 |
| Catálogo (categorías, productos) | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Laboratorio ya integrado; falta activación comercial |
| Modificadores | IMPLEMENTED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | DEFERRED | Mantener persistencia y snapshots |
| Cajas y turnos | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta IPC/UI |
| Ventas | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta IPC/UI |
| Checkout y pagos | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; atómico y probado |
| Inventario y ledger | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta IPC/UI |
| Recetas y efectos de modificador | IMPLEMENTED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | DEFERRED | Mantener resolución; falta administración |
| Proveedores | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | 17 handlers preparados sin registrar |
| Órdenes de compra y transiciones | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta registro comercial |
| Recepción de compras | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; acumulada e idempotente |
| Instalación Local persistente | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | INTEGRATED | BLOCKED | Desktop Windows compila y prueba; E2E gráfico completo pendiente |
| Backup / restauración | IMPLEMENTED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | DEFERRED | Herramienta sin exposición comercial |
| Seguridad Windows (DACL) | IMPLEMENTED | BLOCKED | BLOCKED | BLOCKED | DEFERRED | DEFERRED | Falta crate nativo `pos-windows-security` |
| Docker (runtime Rust) | DEFERRED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | BLOCKED | Arnés smoke; falta flujo comercial dentro del contenedor |
| E2E sintético completo | IMPLEMENTED (Java) | IMPLEMENTED | TESTED | INTEGRATED | DEFERRED | TESTED | `pos-lab` recorre el flujo completo sobre esquema aprobado |
| Control Plane / Pro / SaaS | IMPLEMENTED | DEFERRED | DEFERRED | DEFERRED | DEFERRED | DEFERRED | Fuera de v0.4.0 |
| Stripe / hardware fiscal | IMPLEMENTED | DEFERRED | DEFERRED | DEFERRED | DEFERRED | DEFERRED | Fuera de v0.4.0 |

## Lectura honesta

Con esta matriz **no** se puede retirar Java todavía. La sustitución funcional
existe hasta persistencia y aplicación, pero el producto no es operable por un
usuario final porque faltan:

1. Caso de uso comercial de login/sesión y su IPC.
2. Handlers IPC de operación (organización, catálogo, caja, ventas, inventario,
   compras) y sus ACL por permiso.
3. Pantallas React conectadas a esos handlers.
4. Seguridad nativa de Windows para el canal de provisioning.
5. E2E de escritorio real y Docker que ejerzan el flujo comercial.

## Siguiente orden de corte

1. Login/sesión/RBAC por IPC (desbloquea todas las filas `BLOCKED` de negocio).
2. `pos-windows-security` (desbloquea provisioning en Windows).
3. Handlers de operación + pantallas.
4. E2E de escritorio y Docker comercial.
5. Retirada de Java y release `v0.4.0`.
