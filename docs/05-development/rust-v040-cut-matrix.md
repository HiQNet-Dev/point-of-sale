# Matriz de corte v0.4.0 — HiQNet POS Free Local (Rust)

**Estado a 2026-11-02, tras activar cajas y turnos en el escritorio.**

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
| Organización | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Configuración editable con CAS y auditoría; estado y moneda en solo lectura |
| Sucursales | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Sucursal única: alta inicial, edición y zona horaria triestado |
| Bootstrap administrativo | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Instalación guiada desde el archivo de autorización nativa |
| Provisioning (token/CLI) | IMPLEMENTED (secreto) | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Emisor, lectura y custodia operativos en Unix y Windows ([ADR-0029](../01-architecture/adr/0029-windows-native-file-custody.md)) |
| Usuarios | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Alta con credencial Argon2id, edición con CAS, baja con revocación de sesiones y protección del último administrador |
| Autenticación / sesión | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Sesión nativa, RBAC y reapertura demostrados sobre SQLite real |
| RBAC / memberships | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Gestión de roles, permisos y membresías de sucursal ([ADR-0030](../01-architecture/adr/0030-local-identity-administration.md)) |
| Auditoría | IMPLEMENTED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | DEFERRED | Mantener; no se expone UI en v0.4.0 |
| Catálogo (categorías, productos) | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Operativo: crear, listar, consultar y editar con sesión real |
| Modificadores | IMPLEMENTED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | DEFERRED | Mantener persistencia y snapshots |
| Cajas y turnos | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | IMPLEMENTED | TESTED | Crear caja, abrir, consultar y cerrar turno con CAS, guard de ventas y reapertura ([ADR-0031](../01-architecture/adr/0031-native-branch-selection.md)) |
| Ventas | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta IPC/UI |
| Checkout y pagos | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; atómico y probado |
| Inventario y ledger | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta IPC/UI |
| Recetas y efectos de modificador | IMPLEMENTED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | DEFERRED | Mantener resolución; falta administración |
| Proveedores | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | 17 handlers preparados sin registrar |
| Órdenes de compra y transiciones | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; falta registro comercial |
| Recepción de compras | IMPLEMENTED | IMPLEMENTED | TESTED | BLOCKED | BLOCKED | DEFERRED | Mantener; acumulada e idempotente |
| Instalación Local persistente | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | INTEGRATED | BLOCKED | Desktop Windows compila y prueba; E2E gráfico completo pendiente |
| Backup / restauración | IMPLEMENTED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | DEFERRED | Herramienta sin exposición comercial |
| Seguridad Windows (DACL) | IMPLEMENTED | IMPLEMENTED | TESTED | INTEGRATED | DEFERRED | TESTED | `pos-windows-security`: owner y DACL reales, junctions y sustitución rechazadas |
| Docker (runtime Rust) | DEFERRED | IMPLEMENTED | TESTED | DEFERRED | DEFERRED | BLOCKED | Arnés smoke; falta flujo comercial dentro del contenedor |
| E2E sintético completo | IMPLEMENTED (Java) | IMPLEMENTED | TESTED | INTEGRATED | DEFERRED | TESTED | `pos-lab` recorre el flujo completo sobre esquema aprobado |
| Control Plane / Pro / SaaS | IMPLEMENTED | DEFERRED | DEFERRED | DEFERRED | DEFERRED | DEFERRED | Fuera de v0.4.0 |
| Stripe / hardware fiscal | IMPLEMENTED | DEFERRED | DEFERRED | DEFERRED | DEFERRED | DEFERRED | Fuera de v0.4.0 |

## Lectura honesta

La primera instalación, el catálogo y la administración de identidad ya no
bloquean el producto en ninguna de las dos plataformas soportadas: una persona
configura el negocio, crea el administrador, entra, crea un usuario con su rol,
le da acceso a la sucursal, crea una categoría y un producto, y lo encuentra tras
reiniciar. En Windows la custodia del archivo de autorización, del directorio de
instalación y de los archivos SQLite se verifica contra propietario y DACL
reales; en Unix, contra bits POSIX y propietario efectivo. El E2E de esas filas
es un test de integración Rust sobre el esquema productivo; **no** es un E2E
gráfico de WebView2, que sigue pendiente.

El bloqueo que impedía operar en una sucursal queda resuelto: hasta ahora ningún
usuario podía obtener permisos de sucursal porque la administración de
`branch_membership` no existía. Ahora la membresía se crea y se dota de roles
desde el producto, y `auth_select_branch` resuelve la pierna de sucursal en
consecuencia.

Con esta matriz **no** se puede retirar Java todavía. Faltan:

1. Handlers IPC de ventas, inventario y compras con sus ACL.
2. Pantallas React de esos flujos.
3. E2E gráfico de escritorio y Docker que ejerza el flujo comercial completo.
4. Ratificación de D-AUTH-09 y del perfil de conformidad normativa.
5. Restablecimiento administrativo de contraseña (fuera de alcance de este
   incremento por falta de contrato y de pruebas de seguridad).

## Siguiente orden de corte

1. Ventas y pagos.
2. Inventario y consumo.
3. Compras, backup comercial, instalador, E2E y paridad.
4. Retirada de Java y release `v0.4.0`.

## Extensiones de caja deliberadamente pendientes

La superficie activada cubre el contrato aprobado en `0009`/`0010`: caja, turno,
importe inicial, importe declarado y guard de ventas abiertas. **No** se calculan
movimientos de efectivo, arqueos ni totales esperados porque el esquema y el
dominio todavía no los modelan; añadirlos exige una decisión física nueva.
