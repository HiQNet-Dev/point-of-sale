# Resolución consolidada de autorización física — Incremento 1V

**Estado: pendiente de decisión del titular. Nada de este documento está aprobado.**

Este documento no sustituye al expediente canónico
`docs/05-development/rust-1i-approval-decision-record.md`; lo resume para una
decisión única y añade, por decisión, la recomendación técnica y la consecuencia
de aprobarla. Ninguna migración de `backend-rust/proposals/` se ha promovido a
`backend-rust/migrations/local/`.

## 0. Qué se pide

Una sola aceptación explícita que cubra:

1. **Restauración de contrato** (D-INST-01/02, D-AUTH-01..08, D-AUTH-10,
   D-CAT-01..03, D-CASH-01..03, D-SALES-01/02/05, D-INV-01/02, ADR-0022).
2. **Decisiones que cambian comportamiento** (D-AUTH-11, D-CASH-04,
   D-SALES-03, D-SALES-04), que requieren reconocer el cambio de mecanismo.
3. **Lo que permanece bloqueado**: D-AUTH-09.

Sin esa aceptación, `0002`–`0015` siguen siendo propuestas y `pos-local` sigue
aplicando solo `0001`.

## 1. Decisiones que restauran el contrato congelado

Restaurar significa: la implementación candidata reproduce la ficha o el
esquema Java vigente; no introduce diseño nuevo.

| ID | Contrato original | Implementación candidata | Diferencia | Migración |
| --- | --- | --- | --- | --- |
| D-INST-01 | F1–10 §16.3: estado inicial `ACTIVE` | `0002`: `CHECK (status IN ('ACTIVE'))` | Catálogo de estados incompleto | `0002` |
| D-INST-02 | F1–10 §16.3: huella pública de instalación | `0002`: longitud 1–128, sin `UNIQUE` | Sin ámbito de unicidad | `0002` |
| D-AUTH-01 | F1–10 §16.4: campos de `pos_user` | `0003` transcribe la ficha (`display_name`) | Ninguna | `0003` |
| D-AUTH-02 | F1–10 §16.5: `ACTIVE`/`PASSWORD_RESET_REQUIRED` | `0003` limita el `CHECK` a esos valores | El laboratorio usaba `RESET_REQUIRED`/`DISABLED` | `0003` |
| D-AUTH-03 | F1–10 §16.45: columna `state` | `0003`: `ACTIVE`/`REVOKED`/`EXPIRED`; expiración derivada | Ninguna | `0003` |
| D-AUTH-04 | F1–10 §16.45: ownership vía `pos_user` | `0003` sin `organization_id` en sesión | Ninguna | `0003` |
| D-AUTH-05 | F1–10 §16.5/§16.45: versiones iniciales 1 | `DEFAULT 1` + `CHECK >= 1` | Ninguna | `0003` |
| D-AUTH-06 | V5–V25: catálogo RBAC | `0004` reproduce los 59 códigos | Ninguna | `0004` |
| D-AUTH-07 | F1–10 §17.2: `session_id` nullable | `0006`: USER puede llevar sesión, SYSTEM no | Ninguna | `0006` |
| D-AUTH-08 | F4–14 y V7: append-only | `0006`: triggers `BEFORE UPDATE`/`BEFORE DELETE` | No es inmutabilidad criptográfica | `0006` |
| D-AUTH-10 | F1–10 §17.3: bootstrap por instalación | `0007`: PK `installation_id`, `PENDING`/`COMPLETED` | Ninguna | `0007` |
| D-CAT-01 | V9: `NUMERIC(19,4) >= 0` | `TEXT` canónico escala 4 + `CHECK` | SQLite no tiene decimal exacto | `0008` |
| D-CAT-02 | V9: `categories`/`products` | Singular `category`/`product` | Solo nombre | `0008` |
| D-CAT-03 | Sin acoplamiento a Auth | Sin FKs a `pos_user`/`local_session`/`audit_event` | Ninguna | — |
| D-CASH-01 | V11: caja de sucursal, código único por sucursal | `cash_register` + FK compuesta same-org | Solo nombre | `0009` |
| D-CASH-02 | V12: OPEN/CLOSED, actores same-org | `cash_shift` + FKs compuestas | Depende de `0003` | `0010` |
| D-CASH-03 | V12: `NUMERIC(19,4) >= 0` | `TEXT` canónico escala 4 | Ninguna semántica | `0010` |
| D-SALES-01 | V10: grupos, opciones, asignación | `modifier_*` + `product_modifier_group` | Sin CRUD administrativo | `0011` |
| D-SALES-02 | V13/V19: OPEN/CANCELLED/COMPLETED y snapshots | `sale`, `sale_item`, `sale_item_modifier` con superclave de turno | `COMPLETED` solo alcanzable por checkout | `0012` |
| D-SALES-05 | V14/V19: pagos inmutables y consumo congelado | `payment` + `sale_item_inventory_component` | Ninguna | `0015` |
| D-INV-01 | V18/V20: `NUMERIC(19,6)`, apertura única, ledger append-only | `inventory_item`/`balance`/`ledger` con `TEXT` escala 6 y triggers | Triggers desactivables por un administrador del archivo | `0013` |
| D-INV-02 | V18/V21: receta por producto y efectos por opción | `product_recipe`, `recipe_component`, `modifier_inventory_effect` | Sin mantenimiento administrativo | `0014` |
| ADR-0022 | Design Freeze | Reconciliación 1H→1I documentada como restauración | Ninguna si se clasifica como restauración | — |

**Recomendación técnica:** aprobar el bloque completo. Son restauraciones con
pruebas existentes; aprobarlas no cambia el comportamiento del backend Java ni
introduce diseño nuevo.

**Consecuencia de aprobar:** `0002`–`0015` pueden promoverse a
`migrations/local/` en el orden declarado y sustituir los adaptadores de
laboratorio por los canónicos, sin reescribir seguridad.

## 2. Decisiones que cambian comportamiento o mecanismo

| ID | Contrato original | Implementación candidata | Diferencia real | Riesgo | Migración |
| --- | --- | --- | --- | --- | --- |
| D-AUTH-11 | Java: `toLowerCase(Locale.ROOT)` y unicidad | Normalización Unicode en la aplicación; unicidad sobre el valor canónico | SQLite `lower()` es ASCII; escrituras SQL directas podrían crear duplicados semánticos | Medio: la garantía física es parcial | `0003` |
| D-CASH-04 | Java: `CashShiftService.close()` comparte lock con `SaleService.create()` | Índice parcial `one_open_per_register` + `BEGIN IMMEDIATE` + guard real sobre `sale.status='OPEN'` en la misma UoW | Se sustituye el lock de JVM por serialización de escritor SQLite | Medio: solo válido si toda escritura crítica comparte el mismo mecanismo | `0010` + ventas |
| D-SALES-03 | ADR-0007: lock compartido | Igual que D-CASH-04, verificado con dos pools | Igual | Medio | `0012` |
| D-SALES-04 | ADR-0008/0014: checkout atómico | Pagos + consumo + `COMPLETED` + auditoría en una UoW | Ninguna funcional; cambia el motor transaccional | Bajo técnico | `0015` |

**Recomendación técnica:**

- **D-AUTH-11:** aprobar la normalización obligatoria en la aplicación y dejar
  documentado que SQLite no pliega Unicode por sí sola. No prometer una garantía
  física que el motor no ofrece.
- **D-CASH-04 y D-SALES-03:** aprobar `BEGIN IMMEDIATE` como reemplazo del lock
  de JVM. Es la única estrategia coherente con SQLite y ya está probada con dos
  pools, cancelación y rollback.
- **D-SALES-04:** aprobar el contrato atómico. La activación comercial sigue
  requiriendo D-AUTH-09 resuelta.

**Consecuencia de aprobar:** el cierre de turno y el checkout pasan a depender de
serialización de escritor, no de bloqueos en memoria. Cualquier adaptador futuro
debe usar la misma UoW o el invariante deja de sostenerse.

## 3. Bloqueada expresamente

| ID | Motivo | Qué falta |
| --- | --- | --- |
| D-AUTH-09 | Parámetros Argon2id comerciales | Benchmark en hardware objetivo e interoperabilidad con el verificador Java. Se mantienen solo `LAB_V0`/`LAB_V1` experimentales |

Aprobar D-AUTH-09 sin esa evidencia fijaría una política criptográfica de
producción sin datos. **No se recomienda.**

## 4. Bloqueos por migración

| Migración | Decisiones requeridas | Bloquea |
| --- | --- | --- |
| `0002` installation_identity | D-INST-01, D-INST-02 | — |
| `0003` user/credential/session | D-AUTH-01..05, D-AUTH-11 | `0006` |
| `0004` RBAC | D-AUTH-06 | `0005`, `0007` |
| `0005` memberships | D-AUTH-06 | `0006`, `0007` |
| `0006` audit_event | D-AUTH-07, D-AUTH-08 | cierre del flujo administrativo |
| `0007` bootstrap_record | D-AUTH-10 | provisioning real |
| `0008` catálogo | D-CAT-01..03 | catálogo persistente |
| `0009`–`0010` caja y turnos | D-CASH-01..04 | turnos persistentes |
| `0011`–`0012` modificadores y ventas | D-SALES-01..03 | ventas persistentes |
| `0013`–`0014` inventario y recetas | D-INV-01, D-INV-02 | consumo de stock |
| `0015` checkout | D-SALES-04, D-SALES-05 | venta `COMPLETED` |

Nota: `0008` no depende de `0003`. Es la única familia que podría activarse sin
el paquete D-AUTH, si el titular acepta promociones parciales.

## 5. Estado de la evidencia

| Evidencia | Estado |
| --- | --- |
| Suite Rust local | 276 pruebas, 0 fallos |
| CI Linux del árbol 1U (PR #81) | Técnicos PASS; gate `--require-complete` BLOQUEADO por diseño |
| Flujo sintético extremo a extremo (`pos-lab`) | PASS (local y Docker) |
| Promoción de DDL | **No ejecutada** |

## 6. Bloque de aceptación

Debe completarlo el titular. Sin firma, este paquete no autoriza nada.

```text
Titular: ______________________________
Fecha:   ______________________________

[ ] Apruebo el bloque de restauración (D-INST-01/02, D-AUTH-01..08,
    D-AUTH-10, D-CAT-01..03, D-CASH-01..03, D-SALES-01/02/05, D-INV-01/02,
    ADR-0022).
[ ] Apruebo D-AUTH-11 con la limitación documentada sobre Unicode en SQLite.
[ ] Apruebo D-CASH-04 y D-SALES-03 (BEGIN IMMEDIATE en lugar del lock de JVM).
[ ] Apruebo D-SALES-04 (contrato de checkout atómico).
[ ] Mantengo D-AUTH-09 bloqueada.

Alcance de la aprobación:
  [ ] Completa (todas las migraciones autorizadas)
  [ ] Parcial (indicar cuáles): ______________________________________
```
