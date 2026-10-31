# Tablero maestro de migración Java → Rust

**Promoción física v0.4.0 (2026-10-31).** El esquema `0002`–`0017` dejó de ser
propuesta y vive en `backend-rust/migrations/local/`; ver
[ADR-0026](../01-architecture/adr/0026-local-schema-promotion-v040.md). Este
tablero registra el estado real, no el deseado.

Arquitectura: monolito modular hexagonal (`pos-core` ← `pos-application` ←
`pos-storage-sqlite` ← `pos-local`), documentada en
`docs/01-architecture/rust-hexagonal-architecture.md` y obligatoria en
`AGENTS.md`.

## 1. Escala de estados

| Estado | Criterio mínimo |
| --- | --- |
| `NOT_STARTED` | Sin código Rust propio |
| `DOMAIN` | Agregados, value objects y reglas puras con pruebas |
| `PERSISTENCE` | Repositorio sobre el esquema productivo aprobado (ADR-0026) |
| `APPLICATION` | Casos de uso con autorización, UoW y auditoría transaccional |
| `IPC` | Comando Tauri registrado con DTO generado y ACL |
| `INTEGRATED` | Consumido por React y verificado de extremo a extremo |
| `PARITY_VERIFIED` | Paridad diferencial Java/Rust registrada, incluyendo rechazos |

`PERSISTENCE (lab)` y `PERSISTENCE (fixture)` describen adaptadores que aún usan
las tablas `*_lab` exclusivas de pruebas; no equivalen a persistencia productiva.

## 2. Estado por módulo

| Módulo Java | Estado Rust | Evidencia | Falta |
| --- | --- | --- | --- |
| Organization | `APPLICATION` | `pos-core::organization_branch`, `SqliteOrganizationApplication`, `SqliteUnitOfWork`, pruebas de CAS/rollback/lock sobre `0001` aprobado | IPC y UI canónicos |
| Branch | `APPLICATION` | `BranchApplication`, `SqliteBranchApplication`, pruebas de ownership y timezone | IPC y UI canónicos |
| Audit | `APPLICATION` | `pos-core::audit` (metadata ≤16 KiB, USER/SYSTEM), `0006` aprobado con triggers append-only, `SqliteAuditWriter` transaccional | IPC de consulta |
| Users / Auth | `APPLICATION` | `pos-core::identity`, `0002`–`0004` aprobados, `pos-credentials`; login/logout/sesión comercial por IPC con custodia nativa del handle y revalidación DB-authoritative | Administración de usuarios y UI de recuperación |
| RBAC | `PERSISTENCE` | Catálogo exacto de 59 permisos, override de organización, modo recuperación, resolución contextual probada | IPC de administración de roles |
| Bootstrap | `INTEGRATED` | `pos-application::bootstrap`, credencial inicial comprobada y persistida, organización/administrador y 59 grants + auditoría SYSTEM en una UoW; autorización nativa `0017` leída del archivo privado y consumida en la misma UoW por `bootstrap_install`; UI de primera instalación y login | A/D-AUTH-09 y provisioning Windows |
| Catálogo (categorías, productos) | `INTEGRATED` | `pos-core::catalog`, `0008` aprobado, `SqliteCatalogApplication`, 9 pruebas de integración; IPC comercial `catalog_*` con sesión nativa (listar, consultar, crear categoría/producto, editar con CAS) | Búsqueda de texto y edición de categorías |
| Modificadores | `PERSISTENCE` | `0011` aprobado, FKs same-org y snapshots de opciones usados por ventas SQLite | CRUD/autorización administrativa e IPC |
| Cajas | `APPLICATION` | `pos-core::cash`, `0009` aprobado, `SqliteCashApplication`, SQLite real: scope, CAS, auditoría, reapertura | IPC y pantallas de caja |
| Turnos | `APPLICATION` | `pos-core::cash`, `0010` aprobado, índice parcial OPEN, guard real `sale.status=OPEN`/UoW, dos pools y recuperación | IPC y pantallas de turno |
| Inventario (artículos, saldos, ledger) | `APPLICATION` | `pos-core::inventory`, `0013` aprobado, `SqliteInventoryApplication`, ledger append-only por trigger, apertura única, CAS y dos pools | IPC y pantallas de inventario |
| Ventas | `APPLICATION` | `0011`/`0012` aprobados, `SqliteSalesApplication`, snapshots, CAS, auditoría, carreras venta/cierre y reapertura | IPC y pantallas de venta |
| Checkout y pagos | `APPLICATION` | `0015` aprobado, `SqliteCheckoutApplication`, pagos + consumo + `COMPLETED` + auditoría en una UoW, política de stock y rollback | IPC y pantallas de cobro |
| Compras y recepciones | `APPLICATION` | `pos-core::purchasing` (estados, emisión, cancelación, recepciones acumuladas, costo opcional con prueba «desconocido ≠ cero»), `PurchasingApplication` con 13 casos de uso, `0016` aprobado, 12 pruebas SQLite reales | Registro comercial de los 17 handlers, ACL e interfaz |
| Proveedores | `APPLICATION` | `pos-core::supplier`, `PurchasingApplication` (crear/consultar/listar/actualizar con CAS), `0016` aprobado, pruebas de slug duplicado y aislamiento | IPC y pantallas |
| Recetas y efectos de modificador | `PERSISTENCE` | `pos-core::recipe`, `0014` aprobado, resolución de consumo y snapshots usados por checkout | Mantenimiento administrativo e IPC |
| API HTTP / OpenAPI | `NOT_STARTED` | — | No es requisito de Free Local: el transporte es IPC Tauri |
| Transporte Local | `IPC` | Tauri v2: `bootstrap_*`, `auth_*` y `catalog_*` con handle solo en memoria nativa; [contrato](../03-api/rust-desktop-ipc.md), ADR-0027 y ADR-0028 | Handlers de caja, ventas, inventario y compras |
| Instalación Local persistente | `PERSISTENCE` | `apps/pos-desktop --installation <dir>`: migrador aprobado `0001`–`0017`, clasificación primera instalación/reapertura, rechazo de esquema posterior, cierre con checkpoint; E2E Windows de dos arranques y smoke Docker | Directorio canónico por plataforma (`app_data_dir`) e identificador estable de aplicación |
| Integración React | `INTEGRATED` | Instalación guiada, login, sesión, permisos, sucursal, logout y catálogo operativo sobre IPC | Caja, ventas, inventario y compras |
| Arnés Docker Local | `DEV HARNESS (validated)` | `backend-rust/Dockerfile` multi-stage, runtime no-root, volumen, healthcheck funcional (`verify_health`: integridad + migraciones), reinicio, cierre con checkpoint | No es distribución Free Local ni expone red |
| Arnés sintético extremo a extremo | `E2E (synthetic)` | `tools/pos-lab`: bootstrap → admin → sesión → RBAC → sucursal → categoría → producto → inventario → receta → caja → turno → venta → checkout → consumo → `COMPLETED` → auditoría → cierre → reapertura; matriz ejecutable de dependencias `0002`–`0017` aplicada sobre `0001` con `foreign_key_check` por paso | No es E2E de escritorio ni sustituye la verificación comercial |

### Consolidación 2C: camino persistente, no activación comercial

La instalación aprobada añade validación SQLx de checksums en cada reapertura,
upgrades limitados al migrador aprobado, rechazo de rutas redirigidas/auxiliares
inseguros y exclusión de segunda instancia desktop mediante candado del SO.
Pruebas de la continuación: 318 headless, 35 desktop Windows, 11 de directorios/candado en Linux Docker;
E2E WebView2 de instalación y laboratorio, y smoke Docker aprobados localmente.
Detalle y límites en [el informe 2C](rust-increment-2c.md).

No se promovió ninguna migración ni se habilitó login comercial. Purchasing tiene
17 handlers preparados, DTOs, bindings y servicio/consultas TypeScript, pero su
registro no se incluye en el dispatcher activo ni tiene ACLs. Se corrigió la
idempotencia para exigir misma sucursal, orden y cantidades. No hay paridad
diferencial completa ni pantallas comerciales Rust. A/B/C y D-AUTH-09 siguen pendientes.

### Preparación local 2D — primer arranque seguro

La credencial inicial es obligatoria y debe verificar su contraseña antes de
persistir; no se completa bootstrap sin ella y su auditoría. El adaptador existente
comparte reglas para el físico canónico de credenciales, con prueba de bloqueo
tras reinicio y reset al vencer. Hay snapshots/restauración a destino nuevo, no
herramientas comerciales expuestas. Purchasing conserva sus 17 handlers cerrados.

Validación local: 332 headless, 35 desktop, 220 frontend (+3 contratos), 18 pruebas
SQLite de bootstrap/backup en Linux y 10 checks reales de interoperabilidad
Java/Rust. E2E WebView2 existentes y smoke Docker pasan; no es E2E comercial.

[Informe 2D](rust-increment-2d.md), [matriz A](rust-block-a-readiness-2d.md) y
[ADR-0025](../01-architecture/adr/0025-free-local-first-provisioning.md).

> **Actualización 2026-10-31.** El párrafo histórico 2D anterior queda superado
> en un punto concreto: `0002`–`0017` sí se promovieron y el emisor nativo de
> provisioning ya consume su autorización en la misma UoW del bootstrap
> ([ADR-0026](../01-architecture/adr/0026-local-schema-promotion-v040.md)). Sigue
> sin aprobarse D-AUTH-09 ni la activación comercial de comandos IPC.

### Frontera de sustitución real (no porcentaje de migración)

| Módulo | DOMAIN | APPLICATION | STORAGE | TRANSPORT | REACT | PARITY_VERIFIED |
| --- | --- | --- | --- | --- | --- | --- |
| Identity / RBAC | Sí | Principal/credenciales existentes; login comercial incompleto | Aprobado `0002`–`0004` | Sin login IPC | Java operativo | No |
| Bootstrap | Capacidad y SYSTEM | Canónico, one-shot, grants y contraseña inicial; autorización nativa `0017` | Aprobado `0007`/`0017` | No expuesto | No | No |
| Organization / Branch | Sí | Sí | Aprobado `0001`; atribución canónica pendiente | Lectura de sucursales solo lab | Solo lab | No |
| Catálogo | Sí | Sí | Aprobado `0008` | Lectura/crear producto solo lab | Solo lab | No |
| Caja / turnos | Sí | Sí | Aprobado `0009`–`0010` | No | Java | No |
| Inventario / recetas | Sí | Operaciones de stock; mantenimiento de recetas pendiente | Aprobado `0013`–`0014` | No | Java | No |
| Ventas / checkout | Sí | Sí, UoW única | Aprobado `0011`–`0012` y `0015` | No | Java | No |
| Purchasing (17 operaciones) | Sí | Sí | Aprobado `0016` | 17 handlers preparados, no registrados | Servicio y Query preparados, sin pantallas activadas | No |

Correspondencia de las 17 operaciones, permisos, requests/responses y pruebas:
[Purchasing IPC preparado](../03-api/rust-purchasing-ipc.md). Un handler preparado
no cambia por sí solo el estado de exposición o la autoridad productiva.

## 3. Dinero y cantidades

| Capacidad | Estado |
| --- | --- |
| `Currency` (sintaxis, no ISO 4217) | Implementado |
| `MoneyMinor` (`i64`, suma checked, moneda explícita) | Implementado |
| Importación decimal exacta con escala explícita | Implementado |
| `Decimal4` de catálogo (escala fija 4, rango `NUMERIC(19,4)`) | Implementado (1J) |
| Puente `MoneyMinor` ↔ `Decimal4` con exponente explícito y error ante pérdida | Implementado (1Q, 9 pruebas) |
| Cantidades y conversión de unidades | Cantidades exactas escala 6 implementadas; conversión entre unidades pendiente |
| Redondeo e impuestos | Pendiente |
| Política de stock negativo | Aplicada en checkout (`STRICT`/`ALLOW_NEGATIVE`) sobre saldos reales |

## 4. Bloqueos normativos activos

| Bloqueo | Alcance | Efecto |
| --- | --- | --- |
| D-AUTH-01..08, D-AUTH-10, D-AUTH-11 | Identidad, sesión, auditoría, bootstrap, email | **Resueltas** por ADR-0026; `0002`–`0007` promovidas con la opción A de D-AUTH-11 (normalización en aplicación) |
| D-AUTH-09 | Argon2id comercial | **MEDIDA** el 2026-10-31 en Windows (32 MiB/t=3/p=1, p95 ≈ 51 ms); propuesta `local-argon2-v2` pendiente de ratificación, ver `rust-auth-benchmark-d-auth-09.md` |
| D-INST-01, D-INST-02, ADR-0022 | Instalación y reconciliación física | **Resueltas** por ADR-0026 (`status` restringido a `ACTIVE`, sin unicidad global de huella) |
| Representación física del catálogo | Importes, nombres de tabla | **Resuelta** por ADR-0026 (`0008` promovida) |
| Cajas y turnos | Nombres, importes, índice parcial y coordinación venta/cierre | **Resuelta** por ADR-0026 (`0009`–`0010` promovidas) |
| Ventas y modificadores | Tablas, snapshots, dinero `TEXT` exacto y coordinación SQLite | **Resuelta** por ADR-0026 (`0011`–`0012` promovidas) |
| Inventario, recetas y checkout | Cantidades `TEXT` escala 6, triggers, pagos y snapshots de consumo | **Resuelta** por ADR-0026 (`0013`–`0015` promovidas) |
| Compras | `0016` sobre `unit_cost` opcional y recepciones sin valoración | **Resuelta** por ADR-0026 (`0016` promovida); `ADR-0021` queda implementada en su forma física |

### Decisiones aceptadas

| Decisión | Fecha | Alcance aceptado | Efecto en el tablero |
| --- | --- | --- | --- |
| Bloque D | 2026-10-30 | Tauri v2/IPC, una SQLite por instalación, directorio privado del SO, rutas explícitas solo por argumentos, sin HTTP local, separación laboratorio/producto | Habilita implementar `app_data_dir`; no autoriza distribución ni release |
| `CC21-03` | 2026-10-30 | `unit_cost TEXT NULL` con semántica desconocido ≠ cero y total derivado `NULL` | Implementada en `0016_purchasing.sql` |
| Promoción física `0002`–`0017` | 2026-10-31 | Esquema Local completo inmutable, con D-AUTH-11 opción A | [ADR-0026](../01-architecture/adr/0026-local-schema-promotion-v040.md); el migrador productivo publica 17 versiones |

Ningún bloqueo autoriza a debilitar FKs, `--require-complete` ni los gates
DDL13/PAR/AS.

## 5. Regla de actualización

Un módulo solo avanza cuando el incremento correspondiente publica código,
pruebas y documentación. Los estados `DOMAIN` y `PERSISTENCE (lab|fixture)` no
habilitan operar en producción ni sustituyen una aprobación normativa.
