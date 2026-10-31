# Incremento 2B — Compras completas en Rust, admisión de verificación y bloque D

**Rama:** `feat/fullstack/rust-desktop-vertical-1x` (PR #84 Draft → `dev`).
**Versión:** `0.4.0-SNAPSHOT` sin cambios. Sin merges, sin tags, sin ejecuciones
de Actions (`[skip actions]`).

Este incremento implementa funcionalidad real sobre el código existente. No
rediseña la arquitectura hexagonal ni promueve migraciones no aprobadas.

## 1. Decisiones autorizadas y su registro

El titular aceptó el **2026-10-30** (documento `00B · Control maestro de
revisiones y aprobaciones — HiQNet POS`):

| Decisión | Alcance | Registro en el repositorio |
| --- | --- | --- |
| Bloque D | Tauri v2/IPC, una SQLite por instalación, directorio privado del SO, ruta explícita solo para desarrollo/recuperación, sin HTTP local, separación laboratorio/producto | [ADR-0024](../01-architecture/adr/0024-local-persistent-installation.md), [paquete 1Y](rust-increment-1y-approval-package.md), [tablero](rust-migration-board.md) |
| `CC21-03` | `unit_cost TEXT NULL`; `NULL` ≠ `0.0000`; total derivado `NULL`; reportes distinguen «sin costo»; sin inferir costo efectivo | [CC-0021](rust-adr0021-change-control.md), [resolución 2A](rust-adr0021-resolution-2a.md), [tablero](rust-migration-board.md) |

`ADR-0021` continúa `PROPOSED`: la aceptación parcial **no** cubre las cinco
fichas físicas ausentes, costeo efectivo de recepción, valoración histórica,
regularización, snapshots pendientes ni import.

Ninguna migración se promovió a `migrations/local`. `0016` sigue en
`proposals/local-purchasing/`.

## 2. Compras: 17 operaciones implementadas

`pos-application::purchasing::PurchasingApplication` sobre el dominio existente
`pos-core::purchasing` (estados, transiciones irreversibles, snapshots,
sobre-recepción y costo opcional).

| # | Operación | Permiso | Notas |
| --- | --- | --- | --- |
| 1 | `create_supplier` | `purchasing.supplier.write` | Slug inmutable, contacto validado |
| 2 | `get_supplier` | `purchasing.supplier.read` | Scope de organización |
| 3 | `list_suppliers` | `purchasing.supplier.read` | Página acotada, filtro por estado |
| 4 | `update_supplier` | `purchasing.supplier.write` | CAS por versión |
| 5 | `create_purchase_order` | `purchasing.order.write` | `DRAFT` con partidas y snapshots |
| 6 | `get_purchase_order` | `purchasing.order.read` | Scope de sucursal |
| 7 | `list_purchase_orders` | `purchasing.order.read` | Página acotada, filtro por estado |
| 8 | `change_purchase_order_supplier` | `purchasing.order.write` | Solo `DRAFT`; recaptura snapshots |
| 9 | `add_purchase_order_line` | `purchasing.order.write` | Valida artículo, unidad y cantidad |
| 10 | `update_purchase_order_line` | `purchasing.order.write` | Solo `DRAFT`; costo opcional |
| 11 | `remove_purchase_order_line` | `purchasing.order.write` | Solo `DRAFT` |
| 12 | `issue_purchase_order` | `purchasing.order.issue` | `DRAFT → ORDERED` irreversible |
| 13 | `cancel_purchase_order` | `purchasing.order.cancel` | `DRAFT`/`ORDERED` → `CANCELLED` |
| 14 | `post_purchase_receipt` | `purchasing.receipt.write` | Stock + ledger + estado + auditoría en una UoW |
| 15 | `get_purchase_receipt` | `purchasing.receipt.read` | |
| 16 | `list_purchase_receipts` | `purchasing.receipt.read` | |
| 17 | `purchase_receipt_status` | `purchasing.receipt.read` | Recibido/pendiente por partida |

Garantías conservadas: actor derivado del `VerifiedPrincipal`, autorización
DB-authoritative, CAS, aislamiento organización/sucursal, una sola
`SqliteUnitOfWork` por mutación, auditoría en la misma transacción y ledger
append-only.

### Puerto y adaptador

`PurchasingTransaction: AuditAppender` declara las operaciones que el caso de uso
necesita (≈28 métodos) y `PurchasingUnitOfWork` la transacción. El adaptador vive
en `pos-storage-sqlite::purchasing` con `queries`, `mapper` y `repository`; el SQL
nunca se escribe en la capa de aplicación.

### Esquema candidato

`proposals/local-purchasing/0016_purchasing.proposed.sql` crea `supplier`,
`purchase_order`, `purchase_order_line`, `purchase_receipt` y
`purchase_receipt_line` con FKs compuestas por organización, superclaves de scope
y `unit_cost` nullable según `CC21-03`. Entra en la matriz de dependencias del
Migration Lab (`0016` incluida) sin promoverla.

## 3. Errores corregidos durante la integración

Estos fallos aparecieron al ejecutar las pruebas reales de SQLite, no en revisión
teórica:

1. **Partidas no persistidas.** `create_purchase_order` construía las partidas y
   las devolvía en la vista sin insertarlas. Cualquier `get_purchase_order`
   posterior leía cero partidas. Corregido insertando cada partida dentro de la
   misma UoW.
2. **Escala del total.** El total se calculaba con costo en escala 6 y cantidad en
   escala 6, formateando el producto como escala 10: `2 × 1.5` daba `300.0000000000`.
   Ahora el costo se convierte con su escala real (4) y el producto queda
   exactamente en escala 10 (`3.0000000000`), sin redondeo ni coma flotante.
3. **Lint de tests.** El módulo de pruebas de `purchasing/receipt.rs` usaba
   `panic!` en fixtures; se declara la excepción local como en el resto de
   módulos de prueba.

## 4. Pruebas de compras

12 pruebas de integración sobre SQLite real (`purchasing_store_tests`):

- Ciclo de vida de proveedor con reapertura.
- Slug duplicado y aislamiento entre organizaciones.
- Snapshots de artículo/proveedor y **costo desconocido ≠ cero**.
- Cero conocido persistido como `0.0000`, no `NULL`.
- Edición de partidas solo en `DRAFT` y emisión irreversible (incluye cambio de
  proveedor con CAS).
- Versión obsoleta rechazada y cancelación final.
- Recepción: stock, ledger y auditoría en un único commit.
- Idempotencia por clave y sobre-recepción rechazada.
- Recepción sobre orden no emitida rechazada.
- Rollback completo cuando falla la auditoría.
- Concurrencia de proveedores y visibilidad cruzada de órdenes.

## 5. D-AUTH-09 — evidencia nueva

Se añadió aislamiento de memoria por proceso (`--memory-isolation`) y se ejecutó
en Linux con `--cpus 2 --memory 2g`:

| Escenario | Pico RSS |
| --- | --- |
| Proceso base | ≈2.2 MiB |
| Una operación Argon2id | ≈21.8 MiB |
| 2 verificaciones | 41.6 MiB |
| 4 verificaciones | 80.4 MiB |
| 8 verificaciones | 158.3 MiB |

El costo criptográfico real es ≈19.5 MiB por verificación; la cifra de ≈1.46 GiB
de 2A acumulaba candidatos de 64 MiB y rondas en el mismo proceso. Detalle y
corrección en [evidencia 2B](rust-auth-benchmark-2b.md).

Se implementó la política de admisión en `VerificationGate`: cola acotada, espera
máxima, saturación segura (`IdentityError::VerificationSaturated`) y liberación de
la reserva al cancelar. Pruebas nuevas: 4. `D-AUTH-09` **sigue bloqueada** hasta
medir en hardware objetivo y obtener aprobación.

## 6. Bloque D — implementación autorizada

- Nuevo modo `--installation-default`: la instalación vive en el directorio de
  datos privado del usuario (`%LOCALAPPDATA%`, `$XDG_DATA_HOME` o
  `~/.local/share`) bajo el directorio estable `com.hiqnet.pos`.
- El identificador del bundle **no** se cambió: `com.hiqnet.pos` es el candidato
  documentado y usarlo como *bundle identifier* exige verificar colisiones,
  continuidad de almacenamiento y distribución.
- `--installation <directorio>` sigue disponible para desarrollo y recuperación
  administrativa. Ningún comando acepta rutas desde JavaScript.
- Se conservan las reglas previas: rechazo de archivo/ruta vacía, enlaces
  simbólicos, junctions de Windows, directorio escribible por todos; creación con
  `0700` en Unix; sin borrar ni reemplazar archivos existentes.
- La ruta saliente no se valida a mano: pasa por
  `InstallationDirectory::resolve`, de modo que un directorio de datos del sistema
  comprometido se rechaza igual que una ruta explícita.

## 7. Estado de las suites

| Suite | Resultado |
| --- | --- |
| Rust headless (`cargo test --workspace --all-features`) | 310 pruebas, 0 fallos |
| Escritorio (`apps/pos-desktop`) | 24 pruebas |
| Frontend (`pnpm test`) | 212 aprobadas, 3 omitidas |
| Contratos Rust ↔ TypeScript | 3 pruebas |
| E2E Tauri/WebView2 (laboratorio) | PASS |
| E2E instalación persistente (Windows) | PASS |
| Docker smoke (`smoke-local.mjs`) | PASS, first_install → reopened, checksum idéntico |

`cargo fmt --check`, `cargo clippy -D warnings` y `cargo build` limpios en el
workspace headless y en el de escritorio.

## 8. Qué sigue bloqueado

| Bloque | Estado | Efecto |
| --- | --- | --- |
| A — Identidad, sesiones, RBAC, auditoría | No aprobado | Sin login comercial ni sesiones productivas |
| B — Catálogo, cajas, turnos | No aprobado | Sin promoción de `0008`–`0010` |
| C — Ventas, inventario, checkout | No aprobado; depende de `ADR-0021` | Sin promoción de `0011`–`0015` |
| Compras (`0016`) | Implementado sobre esquema candidato | Requiere aprobación física para promover |
| `D-AUTH-09` | Bloqueada | Falta hardware objetivo y aprobación |
| Casos normativos | 1 aprobado, 28 bloqueados, 1 sin ejecutar | Sin cambio: ningún gate se relajó |

## 9. Siguiente vertical

Con identidad y esquema aprobados, la secuencia inmediata es:

```text
--installation-default
→ Bootstrap autorizado → Login real → Sesión revocable → RBAC
→ Sucursal → Categoría → Producto → Caja → Turno
→ Venta → Pago → Inventario → COMPLETED → Auditoría
→ Reinicio → Persistencia verificada
```

Compras se conectará a IPC/React después de esa vertical y solo cuando `0016`
esté aprobada, para no exponer una pantalla operativa sobre esquema candidato.
