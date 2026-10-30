# Incremento 1T — inventario, recetas y checkout candidatos

**Estado:** candidato, no productivo. Ninguna migración se promovió a
`backend-rust/migrations/local/` y ninguna decisión normativa se marcó como
aprobada.

## 1. Alcance entregado

| Frente | Entregable |
| --- | --- |
| Persistencia de inventario | `0013_inventory.proposed.sql`, `SqliteInventoryApplication` |
| Recetas y efectos | `0014_inventory_recipes.proposed.sql`, lectura transaccional para consumo |
| Checkout | `0015_checkout.proposed.sql`, `SqliteCheckoutApplication` |
| Permisos | Seis códigos canónicos de inventario añadidos a `Permission` (ya existían en V5–V25 y en el catálogo de 59) |
| Docker | `backend-rust/Dockerfile`, `docker/healthcheck.sh`, `docker-compose.rust.yml` |
| Operación Git | PR Draft apilado sobre #78 |

## 2. Operaciones ejecutables en pruebas

Inventario:

- Crear artículo con código único por organización.
- Apertura única por sucursal y artículo, con cantidad exacta escala 6.
- Ajuste manual con motivo obligatorio y CAS sobre el saldo.
- Lectura de saldo y del ledger histórico.
- Congelación de la unidad base en el primer movimiento.
- Ledger append-only por trigger (`UPDATE`/`DELETE` rechazados).

Checkout:

- Venta `OPEN` → validación de total recalculado → pagos → snapshots de consumo
  → movimientos de ledger → `COMPLETED` → auditoría → **commit único**.
- Política de stock `STRICT` (todo o nada) y `ALLOW_NEGATIVE`.
- Efectos de modificador acumulados sobre la receta de la línea.
- Rechazo de segundo checkout, de pagos que no suman el total y de consumo sin
  receta activa válida.
- Rollback completo si la auditoría falla.

## 3. Evidencia ejecutada (Windows, local)

```text
cargo fmt --all -- --check                                              PASS
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings  PASS
cargo test --locked --workspace --all-features                          PASS
cargo build --locked --workspace --all-features                         (ver informe)
```

Recuento real de la suite: **27 + 125 + 98 + 8 + 11 = 269 pruebas aprobadas**
(`migration-lab` incluye 25 unitarias y 2 de integración).

Pruebas nuevas:

- `inventory_store_tests`: 4 (apertura/ajuste/reapertura, reglas y unicidad,
  dos aperturas concurrentes, ajustes concurrentes sin pérdida).
- `checkout_store_tests`: 5 (flujo completo con modificador, stock estricto y
  política negativa, segundo checkout y suma de pagos, rollback por auditoría,
  checkout frente a cierre de turno).

No se ejecutaron Linux/WSL2, Migration Lab PostgreSQL 17 ni GitHub CI para este
incremento. No se declaran.

## 4. Docker

Validado con Docker 29.9.2 (Linux):

- `docker compose -f docker-compose.rust.yml build` → imagen construida.
- `up -d` → contenedor `healthy`; log `{"event":"ready"}`.
- Usuario de ejecución `uid=10001(pos)`, rootfs de solo lectura, volumen propio.
- `stop` → salida `0` con `shutdown_requested` y `stopped`.
- Reinicio → checksum de `pos.sqlite` idéntico (`50cae0fc…`) y migración `0001`
  presente: `migrate()` es idempotente y el volumen persiste.

El contenedor no expone puertos y **no** define la distribución Free Local.

## 5. Límites deliberados

- `0013`–`0015` no viven en `migrations/local` ni los descubre `pos-local`.
- No hay administración de recetas ni de artículos por transporte; las recetas se
  siembran directamente en fixtures de prueba.
- No hay API HTTP/IPC, ni integración React, ni autenticación comercial.
- No se retiraron adaptadores `*_lab` ni se conectó el composition root
  productivo a tablas candidatas.
- D-AUTH-09 sigue bloqueada para parámetros comerciales de Argon2id.

## 6. Decisiones pendientes añadidas

`D-INV-01`, `D-INV-02`, `D-SALES-04` y `D-SALES-05`, registradas como
recomendaciones en `rust-1i-approval-decision-record.md`. Ninguna está aprobada.

## 7. Siguiente paso

Con aprobación física: promover `0002`–`0015` en orden, sustituir los
adaptadores de laboratorio por los canónicos sobre las tablas reales y habilitar
el primer flujo administrativo. Sin aprobación: mantenimiento administrativo de
recetas/artículos sobre el esquema candidato y preparación del adaptador de
transporte Local aprobado (Tauri/IPC), sin exponer red.
