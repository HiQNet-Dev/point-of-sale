# Incremento 1Z — desbloqueo técnico de Rust Local

**Estado: implementado y validado localmente.** Sin aprobaciones normativas, sin
merge, sin release y sin retirar Spring Boot. Se añade evidencia nueva para
D-AUTH-09 y se prepara la primera escritura auditada.

Base: rama `feat/fullstack/rust-desktop-vertical-1x`, HEAD `4f1eb723`, PR #84
(Draft, base `dev`). Esta entrega continúa sobre la misma rama y el mismo punto de
integración.

## 1. D-AUTH-09 — evidencia criptográfica

Instrumentos nuevos y reproducibles:

- `pos-credentials/examples/argon2_bench.rs`: benchmark con entorno (CPU, memoria,
  SO), ruta pública real, candidatos fuera de política, concurrencia, RSS pico en
  Linux y JSON verificable.
- `tools/credential-interop/`: interoperabilidad **bidireccional** con el backend
  Java real (`BCryptPasswordEncoder` y `Argon2PasswordEncoder` de
  `spring-security-crypto`, sin red).
- `pos-credentials/examples/credential_interop.rs`: lado Rust del intercambio.

Resultados (detalle y tablas en
[`rust-auth-benchmark-1z.md`](rust-auth-benchmark-1z.md)):

| Evidencia | Resultado |
| --- | --- |
| `lab-argon2-v1` (19 456 KiB, t=2, p=1) verificación | 17.3 ms (Windows), 17.8 ms (Linux/Ryzen 5 5600G) |
| Concurrencia 8 verificaciones simultáneas | 0 fallos; 5.95 ms por operación en Windows |
| Java BCrypt (coste 10/12) → Rust | `VERIFIED_REHASH` |
| Rust Argon2id → Java `Argon2PasswordEncoder` | `true` con la contraseña correcta, `false` con otra |
| Valor por defecto de Spring (46 MiB) | **Fuera** del techo de política (32 768 KiB) |

Recomendación propuesta: `Argon2id, m=19456, t=2, p=1`, techo 32 768 KiB y
renombrar la política al aprobarla. **D-AUTH-09 sigue bloqueada** hasta medir en
hardware objetivo de PyME y contar con aprobación explícita.

## 2. Bloque D — política de instalación y transporte

Resolución propuesta y añadida a [ADR-0024](../01-architecture/adr/0024-local-persistent-installation.md):
Tauri/IPC como transporte, una SQLite por instalación, directorio privado del
sistema operativo, rutas explícitas solo para desarrollo/recuperación, sin HTTP
local y con separación estricta laboratorio/producto.

Implementado en 1Z (sin DDL):

- `apps/pos-desktop/src/installation_dir.rs`: valida el directorio antes de abrir
  SQLite; rechaza archivos, enlaces simbólicos y directorios escribibles por
  todos; crea con `0700` en Unix; nunca borra el archivo existente.
- La ruta por plataforma (`app_data_dir`) y el identificador de aplicación siguen
  pendientes: son decisiones de identidad de producto (`D-INST`).

## 3. Bootstrap SYSTEM — resolución técnica

No se añadió un segundo camino. La resolución canónica queda documentada y
verificada por las pruebas existentes:

| Elemento | Regla |
| --- | --- |
| `bootstrap.completed` | Único evento con `actor_type = SYSTEM`; exige `BootstrapProvisioning`, que solo produce `authorize_bootstrap` tras validar el secreto de despliegue |
| Sesión | Un evento SYSTEM no lleva `session_id` ni `actor_user_id`; el esquema lo impone con `CHECK` |
| Eventos USER | Exigen `VerifiedPrincipal`; el actor se deriva del principal, nunca del cliente |
| Organización inicial | Se crea dentro de la misma UoW del bootstrap, antes de la instalación |
| Segundo intento | Rechazado por `bootstrap_record` (`PENDING` → `COMPLETED` una sola vez) |
| Alcance | Ningún comando IPC puede provisionar (probado: el comando inexistente se rechaza) |

El camino `*_lab` sigue siendo exclusivo de `pos-lab`; no se sustituyó en el
composition root porque `0002`–`0007` no están aprobadas.

## 4. Migraciones — matriz ejecutable

Se añadió `migration_set_is_dependency_ordered_and_referential` en `pos-lab`:
aplica una propuesta a la vez sobre una base vacía, calcula el delta de
`sqlite_master`, comprueba que cada FK apunta a una tabla existente y ejecuta
`pragma_foreign_key_check` tras cada paso.

| Migración | Objetos que aporta |
| --- | --- |
| `0002` | `installation_identity` |
| `0003` | `pos_user`, `local_credential`, `local_session` + 3 índices |
| `0004` | `permission`, `role`, `role_permission`, `user_organization_role` + 2 índices |
| `0005` | `branch_membership`, `branch_membership_role` + 2 índices |
| `0006` | `audit_event` + 2 triggers append-only + 4 índices |
| `0007` | `bootstrap_record` |
| `0008` | `category`, `product` + 4 índices (incluido barcode único) |
| `0009` | `cash_register` + 1 índice |
| `0010` | `cash_shift` + índice parcial `cash_shift_one_open_per_register` |
| `0011` | `modifier_group`, `modifier_option`, `product_modifier_group` + 3 índices |
| `0012` | `sale`, `sale_item`, `sale_item_modifier` + 3 índices |
| `0013` | `inventory_item`, `inventory_balance`, `inventory_ledger` + 2 triggers append-only + 2 índices |
| `0014` | `product_recipe`, `recipe_component`, `modifier_inventory_effect` + 2 índices |
| `0015` | `payment`, `sale_item_inventory_component` + 2 índices |

Ninguna propuesta se promovió a `migrations/local`.

## 5. Primera escritura de producto (preparada y demostrada en laboratorio)

Nuevo comando `lab_create_product` con el caso de uso ya existente
(`CatalogApplication::create_product`), no una implementación paralela:

```text
React (formulario) → desktop.generated.ts → IPC Tauri → commands.rs
  → LocalState::create_product → CatalogApplication (pos-application)
  → SqliteUnitOfWork + SqliteAuditWriter → SQLite
  → CreatedProductDto → React (lista refrescada)
```

Propiedades verificadas:

- Permiso `product.create` resuelto desde el principal verificado.
- Organización derivada del principal; el request no la acepta.
- Precio decimal exacto (texto escala 4); `9.9900` se conserva literal.
- Auditoría `product.created` en la misma transacción (probado por `entity_id`).
- SKU duplicado → `CONFLICT`; precio con cinco decimales o SKU vacío →
  `VALIDATION_ERROR`.
- Modo instalación → `OPERATION_NOT_AVAILABLE` sin consultar SQL.
- Cross-org: el listado sigue sin devolver datos de otra organización.

**No se expone en modo producto**: el esquema de catálogo, la identidad comercial
y la auditoría productiva siguen pendientes de aprobación.

## 6. Casos normativos

Sin cambios de estado: **1 PASSED (PAR-08), 28 BLOCKED, 1 NOT_RUN**. La
clasificación y el aporte parcial a PAR-05/AS-09 están en
[`rust-increment-1z-normative.md`](rust-increment-1z-normative.md).

## 7. Validación ejecutada

| Ámbito | Comando | Resultado |
| --- | --- | --- |
| Workspace headless | fmt, clippy `-D warnings`, `cargo test --workspace`, `cargo build` | **292 pruebas**, 0 fallos |
| Desktop | fmt, clippy `-D warnings`, `cargo test` | **21 pruebas**, 0 fallos |
| Frontend | lint, `tsc --noEmit`, `vitest` | **212 pruebas**, 3 opcionales omitidas |
| E2E laboratorio (Windows/WebView2) | `pnpm test:desktop-lab` | PASS, incluida la escritura y el conflicto |
| E2E instalación (Windows/WebView2) | `pnpm test:desktop-installation` | PASS (primera instalación → reabierta) |
| Docker Linux | `node backend-rust/docker/smoke-local.mjs` | PASS (`first_install`/`reopened`, hash `21e160d4…`) |
| Evidencia criptográfica | benchmark + interop | PASS (ver documento específico) |

Conteo anterior: 290 workspace / 15 desktop / 211 frontend. Las diferencias
corresponden a las pruebas nuevas de matriz de migraciones, directorio de
instalación, escritura auditada y formulario React.

## 8. Bloqueos vigentes

| Bloqueo | Efecto |
| --- | --- |
| Bloque A (D-AUTH/D-INST) | Sin login, sesiones ni RBAC de producto |
| Bloque B (D-CAT/D-CASH) | Sin catálogo ni cajas persistentes de producto |
| Bloque C (D-SALES/D-INV) | Sin ventas, inventario ni checkout de producto |
| `D-AUTH-09` | Sin parámetros comerciales; falta hardware objetivo |
| ADR-0021 | Migración de datos de compras bloqueada |
| CI remoto | Requiere autorización para reanudar Actions |

## 9. Siguiente paso

Con el bloque A firmado: promover `0002`–`0007`, sustituir la sesión sintética por
verificación de credenciales real y repetir la matriz de migraciones sobre el
migrador aprobado. Con el bloque B: exponer `create_product` en producto con CAS
y auditoría, reutilizando exactamente el comando ya probado.
