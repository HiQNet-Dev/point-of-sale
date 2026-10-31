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
| Bootstrap administrativo | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Instalación guiada desde el archivo de autorización nativa |
| Provisioning (token/CLI) | IMPLEMENTED (secreto) | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Token leído solo por Rust; Windows fail-closed |
| Usuarios | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Login operativo; falta administración de usuarios |
| Autenticación / sesión | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Sesión nativa, RBAC y reapertura demostrados sobre SQLite real |
| RBAC / memberships | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; resolución probada |
| Auditoría | IMPLEMENTED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | DEFERRED | Mantener; no se expone UI en v0.4.0 |
| Catálogo (categorías, productos) | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Operativo: crear, listar, consultar y editar con sesión real |
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

La primera instalación y el catálogo ya no bloquean el producto: una persona
configura el negocio desde el asistente, crea el administrador, entra, crea una
categoría y un producto, y lo encuentra tras reiniciar. El E2E de esas filas es
un test de integración Rust sobre el esquema productivo; **no** es un E2E
gráfico de WebView2, y la instalación guiada sigue deshabilitada en Windows.

Con esta matriz **no** se puede retirar Java todavía. Faltan:

1. Handlers IPC de caja, ventas, inventario y compras con sus ACL.
2. Pantallas React de esos flujos.
3. Seguridad nativa de Windows (DACL/owner) para provisioning y archivos.
4. E2E gráfico de escritorio y Docker que ejerza el flujo comercial completo.

## Siguiente orden de corte

1. Organización y administración de sucursales por IPC/UI.
2. Caja y turnos.
3. Ventas y pagos.
4. Inventario y consumo.
5. Compras, backup comercial, instalador, E2E y paridad.
6. Retirada de Java y release `v0.4.0`.
