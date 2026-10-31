# Paquete de decisión 1Y — activación condicionada del backend Rust Local

**Estado vigente: A/B/C pendientes. D aceptado como diseño el 2026-10-30;
CC21-03 aceptada parcialmente. D-AUTH-09 sigue bloqueada.** Las aceptaciones
no autorizan por sí solas despliegue, merge o release.

Preparación actual de A y decisión adicional de primer arranque:
[matriz 2D](rust-block-a-readiness-2d.md) y
[ADR-0025 propuesto](../01-architecture/adr/0025-free-local-first-provisioning.md).
No registran nuevas aprobaciones.

Este paquete no sustituye al expediente canónico
[`rust-1i-approval-decision-record.md`](rust-1i-approval-decision-record.md) ni a la
resolución de 1V [`rust-1v-approval-resolution.md`](rust-1v-approval-resolution.md):
los reorganiza en cuatro bloques de firma para que el titular pueda aceptar o
rechazar unidades coherentes sin revisar una matriz de 40 entradas cada vez.

## 0. Reglas de lectura

1. **Aprobar un esquema no autoriza merge, despliegue ni release.** La activación
   comercial sigue exigiendo pruebas, CI y aprobación de release.
2. Aprobar un bloque habilita únicamente las migraciones que ese bloque enumera.
3. `D-AUTH-09` no se puede aprobar por firma: requiere benchmark en hardware
   objetivo e interoperabilidad criptográfica con el verificador Java.
4. Ninguna propuesta se promueve a `migrations/local` sin la casilla del bloque
   correspondiente marcada por el titular.
5. Este documento distingue **restauración** (el candidato reproduce el contrato
   congelado) de **cambio de comportamiento** (el mecanismo difiere y hay que
   reconocerlo explícitamente).

## 1. Bloque A — Identidad, sesiones, RBAC, auditoría e instalación

**Migraciones:** `0002_installation_identity`, `0003_user_credential_session`,
`0004_rbac_catalog`, `0005_branch_membership`, `0006_audit_event`,
`0007_bootstrap_record`.

**Decisiones incluidas**

| ID | Tipo | Resumen | Riesgo residual |
| --- | --- | --- | --- |
| `D-INST-01` | Restauración | Estado inicial de instalación `ACTIVE` | Catálogo de estados incompleto |
| `D-INST-02` | Restauración | Huella pública de instalación sin ámbito de unicidad | Sin `UNIQUE`: dos filas podrían compartir huella |
| `D-AUTH-01`..`D-AUTH-05` | Restauración | Campos de `pos_user`, estados, sesión, ownership vía usuario, versiones iniciales 1 | Bajo |
| `D-AUTH-06` | Restauración | Catálogo RBAC de 59 permisos | Bajo |
| `D-AUTH-07` | Restauración | `session_id` nullable; `SYSTEM` sin sesión | Bajo |
| `D-AUTH-08` | Restauración | Auditoría append-only por triggers | No es inmutabilidad criptográfica |
| `D-AUTH-10` | Restauración | Bootstrap por instalación, `PENDING`/`COMPLETED` | Atribución `SYSTEM` pendiente |
| `D-AUTH-11` | **Cambio** | Normalización Unicode del email en la aplicación; unicidad sobre el valor canónico | SQLite `lower()` es ASCII: una escritura SQL directa podría crear un duplicado semántico |
| `D-AUTH-09` | **Bloqueada** | Parámetros Argon2id comerciales | Sin evidencia de benchmark ni interoperabilidad |

**Qué desbloquea**

- Composición canónica de identidad, sesiones y RBAC en `pos-local` y en la
  aplicación de escritorio.
- Sustitución del camino `*_lab` en la ruta de producto.
- Login real con emisión de sesión y resolución de permisos DB-authoritative.

**Qué no desbloquea**

- Credenciales comerciales: `D-AUTH-09` sigue bloqueada. Se puede activar el
  esquema y la verificación con la política que resulte del benchmark, nunca con
  `LAB_V0`/`LAB_V1`.
- Consolidación de los dos caminos Organization/Branch: requiere resolver la
  atribución `SYSTEM` del bootstrap.

**Alternativas consideradas**

- Mantener `RESET_REQUIRED`/`DISABLED` del laboratorio: se rechaza, el contrato
  congelado define `PASSWORD_RESET_REQUIRED`.
- Unicidad sólo física con `lower()`: se rechaza por la limitación ASCII.

## 2. Bloque B — Catálogo, cajas y turnos

**Migraciones:** `0008_catalog`, `0009_cash_register`, `0010_cash_shift`.

| ID | Tipo | Resumen | Riesgo residual |
| --- | --- | --- | --- |
| `D-CAT-01` | Restauración | `NUMERIC(19,4)` → `TEXT` escala 4 con `CHECK` | SQLite no tiene decimal exacto; la validez depende del formato canónico |
| `D-CAT-02` | Cambio menor | Tablas singulares `category`/`product` | Sólo nombre |
| `D-CAT-03` | Restauración | Catálogo sin FK a Auth | Bajo |
| `D-CASH-01` | Cambio menor | `cash_register` con FK compuesta same-org y código único por sucursal | Sólo nombre |
| `D-CASH-02` | Restauración | `cash_shift` con actores same-org | Depende del Bloque A |
| `D-CASH-03` | Restauración | Importes como `TEXT` escala 4 no negativos | Bajo |
| `D-CASH-04` | **Cambio** | `BEGIN IMMEDIATE` + índice parcial de turno abierto sustituyen el lock de JVM | Sólo válido si toda escritura crítica comparte el mecanismo |

**Qué desbloquea**

- Catálogo persistente y administrable con CAS y auditoría.
- Cajas y turnos persistentes con la invariante física
  `cash_shift_one_open_per_register`, probada con dos pools.
- Primera escritura administrativa por IPC (`crear producto`), que hoy está
  implementada en aplicación pero no expuesta.

**Qué no desbloquea**

- Ventas ni cierre con guard real de ventas abiertas: Bloque C.

## 3. Bloque C — Ventas, inventario y checkout

**Migraciones:** `0011_modifier_catalog`, `0012_sales_foundation`,
`0013_inventory`, `0014_inventory_recipes`, `0015_checkout`.

| ID | Tipo | Resumen | Riesgo residual |
| --- | --- | --- | --- |
| `D-SALES-01` | Restauración | Grupos, opciones y asignación de modificadores | Sin CRUD administrativo |
| `D-SALES-02` | Restauración | `sale`, `sale_item`, `sale_item_modifier` con superclave de turno | `COMPLETED` sólo por checkout |
| `D-SALES-03` | **Cambio** | Serialización venta↔cierre con `BEGIN IMMEDIATE` | Depende de `D-CASH-04` |
| `D-SALES-04` | **Cambio** | Checkout atómico en una única UoW | Cambia el motor transaccional |
| `D-SALES-05` | Restauración | Pagos inmutables y snapshots de consumo | Bajo |
| `D-INV-01` | Restauración | `TEXT` escala 6, apertura única, ledger append-only | Triggers desactivables por un administrador del archivo |
| `D-INV-02` | Restauración | Recetas por producto y efectos por opción | Sin mantenimiento administrativo |

**Qué desbloquea**

- Ventas `OPEN` persistentes, partidas, snapshots y totales exactos.
- Guard real de ventas abiertas dentro de la misma `SqliteTransaction` que el
  cierre de turno.
- Checkout con pagos, consumo de inventario y `COMPLETED` en una sola transacción.

**Dependencia normativa adicional**

`ADR-0021` registra conflictos entre el contrato congelado y el esquema candidato.
Antes de migrar datos de producción hay que resolver explícitamente el costo nulo
de compras y las diferencias de columnas señaladas allí. Aprobar este bloque **no**
resuelve `ADR-0021`.

## 4. Bloque D — Transporte de escritorio y política de credenciales

**Aceptado como diseño el 2026-10-30.** No autoriza distribución, instalador ni
release.

| Elemento | Estado | Qué se propone |
| --- | --- | --- |
| `ADR-0023` laboratorio Tauri de sólo lectura | Aceptado como laboratorio en 1X | Se mantiene; el modo laboratorio sigue sin datos reales |
| `ADR-0024` instalación Local persistente (1Y) | **Aceptado como diseño (2026-10-30)** | Modo producto con migraciones aprobadas y estado de instalación; sin identidad sintética ni operaciones de negocio |
| Transporte comercial | **Aceptado**: Tauri v2/IPC | Frontera de producto; no se introduce servidor HTTP local |
| Directorio de datos | **Aceptado**: directorio privado administrado por el sistema operativo | Pendiente de implementar `app_data_dir`; una SQLite por instalación |
| Identificador de aplicación | **Pendiente de fijar** | `com.hiqnet.pos` es candidato; exige verificar colisiones, continuidad de almacenamiento y distribución |
| Rutas desde JavaScript | **Aceptado: prohibidas** | Solo desarrollo y recuperación autorizada, por argumentos |
| `D-AUTH-09` | **Bloqueada** | Benchmark de Argon2id e interoperabilidad con el verificador Java |

**Qué desbloquea**

- Uso documentado y probado del modo instalación sin exponer operaciones no
  autorizadas.
- Base para conectar login real en cuanto se acepten los Bloques A y D.

**Qué no desbloquea**

- Distribución al cliente, instalador, autoarranque, actualizaciones ni releases.

## 5. Orden recomendado de firma

1. **Bloque D (transporte y directorio)** — no depende de esquema y desbloquea la
   decisión de dónde vive la instalación.
2. **Bloque A** — habilita identidad, sesión y RBAC; requiere resolver la
   atribución `SYSTEM` y la política de bootstrap.
3. **Bloque B** — habilita la primera escritura administrativa real.
4. **Bloque C** — habilita ventas y checkout; depende de `D-CASH-04` y de la
   lectura de `ADR-0021`.

Firmar A y B sin C permite operar catálogo y cajas con ventas aún bloqueadas.
Firmar C sin A dejaría ventas sin identidad comercial: no se recomienda.

## 6. Bloque de aceptación

Marcar sólo lo que el titular acepte. Sin marcas, todo permanece pendiente.

```text
[ ] Bloque A — Identidad, sesiones, RBAC, auditoría e instalación (0002–0007),
    incluida D-AUTH-11 con su limitación Unicode documentada.
[ ] Bloque B — Catálogo, cajas y turnos (0008–0010), incluida D-CASH-04
    (BEGIN IMMEDIATE + índice parcial en lugar del lock de JVM).
[ ] Bloque C — Ventas, inventario y checkout (0011–0015), incluidas D-SALES-03,
    D-SALES-04 y la dependencia abierta de ADR-0021.
[x] Bloque D — Transporte Tauri/IPC para producto y política de directorio de
    instalación (ADR-0024). **Aceptado el 2026-10-30** como diseño; excluye
    distribución, instalador y release.
[ ] Mantengo D-AUTH-09 bloqueada hasta disponer de benchmark e interoperabilidad.

**Aprobación parcial adicional registrada el 2026-10-30:** `CC21-03` (costo
pactado `unit_cost TEXT NULL`). No aprueba `ADR-0021` completo ni los bloques A,
B o C.

Firmado por:
Fecha:
Alcance explícitamente excluido de esta firma:
```

## 7. Qué ocurre técnicamente al marcar cada bloque

| Bloque marcado | Acción de activación |
| --- | --- |
| A | Promover `0002`–`0007` a `migrations/local`, componer identidad y RBAC canónicos, retirar `*_lab` de la ruta de producto |
| B | Promover `0008`–`0010`, conectar catálogo/caja canónicos y exponer la primera escritura IPC con CAS |
| C | Promover `0011`–`0015`, conectar ventas, guard real y checkout; resolver antes `ADR-0021` |
| D | Publicar ADR-0024 como aceptado y fijar la política de directorio e identidad de instalación |

Ninguna de estas acciones se ha ejecutado en 1Y.
