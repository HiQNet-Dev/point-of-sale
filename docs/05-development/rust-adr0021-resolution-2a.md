# Resolución técnica de ADR-0021 — Incremento 2A

**Estado: resolución técnica preparada. `CC21-03` aceptada el 2026-10-30;
`ADR-0021` continúa `PROPOSED`.** No modifica el contrato congelado, no crea
fichas canónicas, no promueve DDL y no inventa datos históricos. Su objetivo es
que, cuando el titular decida, la implementación no requiera rediseño.

Relacionado: [ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md),
[CC-0021](rust-adr0021-change-control.md),
[dependencias faltantes](rust-missing-physical-proposal.md).

## 1. Los tres conflictos reales

| Conflicto | Hecho Java | Destino congelado | Efecto |
| --- | --- | --- | --- |
| `unit_cost` | `null` permitido en `ORDERED`/`RECEIVED` | `NOT NULL` | Un importe no informado no es representable sin inventar un valor |
| Fichas padre ausentes | FKs declaradas a `price_book`, `tax_profile`, `costing_policy`, `expense_category`, `plan` | Sin ficha física | Cinco FK sin padre aprobado; `DDL13-01..03` no puede cerrarse |
| Roots | Un modelo lógico | Local / Central / Control | Los tres esquemas deben poder instalarse por separado con los mismos números |

## 2. `unit_cost`: qué ya está resuelto en código

El dominio Rust **ya representa la ausencia de costo pactado**:

```rust
pub struct PurchaseOrderLine { /* … */ unit_cost: Option<UnitPrice> }
```

- `None` significa «no informado por el operador».
- `Some(0.0000)` es un costo cero explícito.
- `restore` conserva el valor tal como se persistió, sin normalizarlo.

Prueba añadida en 2A (`purchasing_tests::agreed_cost_distinguishes_unknown_from_zero`)
que fija esa diferencia: una partida sin costo es válida y **no** equivale a cero.

Conclusión: el conflicto **no está en el dominio**. Está en el DDL congelado, que
declara la columna `NOT NULL`.

## 3. Alternativas físicas para `unit_cost`

| Opción | Representación | Ventajas | Coste / riesgo | Recomendación |
| --- | --- | --- | --- | --- |
| **A** | `unit_cost TEXT NULL` con `CHECK` de formato escala 4 y `>= 0` | Conserva el estado Java; no inventa hechos; el total se deriva explícitamente | Cambia un `NOT NULL` del destino congelado: requiere decisión del titular | **Recomendada** |
| B | `NOT NULL` con centinela (`-1`) | Sin cambio de nulabilidad | Rompe `>= 0`, confunde semántica y envenena agregaciones | Rechazada |
| C | `NOT NULL` con `0` | Sin cambio de nulabilidad | Convierte «desconocido» en «cero»: viola la condición de rechazo de CC-0021 | Rechazada |
| D | `unit_cost_known BOOLEAN` + valor `NOT NULL` | Explícito | Dos columnas para un dato, estados imposibles (`known=false` con valor) | Rechazada |

### Consecuencia de adoptar A

1. El total de la orden **no puede ser `NOT NULL`**: si alguna partida desconoce
   su costo, el total es `NULL` con motivo, no una suma parcial disfrazada.
2. Los reportes deben mostrar «sin costo» de forma explícita; nunca `0.00`.
3. La comparación de paridad acepta `NULL` frente a `NULL` y exige explicar
   cualquier `NULL` frente a valor.
4. Ningún dato histórico se reconstruye: las filas existentes conservan su valor
   o su ausencia.

## 4. Las cinco fichas ausentes

Sin diccionario aprobado no se puede escribir DDL. Lo que falta, por entidad:

| Destino | FK que lo exige | Falta decidir |
| --- | --- | --- |
| `price_book` | `price_version.price_book_id` | Campos, lifecycle, unicidad, scope por organización |
| `tax_profile` | `tax_rule_version.tax_profile_id` | Catálogo inicial, selección, vigencia |
| `costing_policy` | `valuation_entry.costing_policy_id` | Versionado, aplicación por artículo, relación con sucursal |
| `expense_category` | `expense.category_id` | Jerarquía, estado, política contable |
| `plan` | `plan_version.plan_id` | Alcance (cliente/tenant), tarifas, independencia de Free |

La propuesta existente (`rust-missing-physical-proposal.md`) describe campos y
reglas comunes, pero está **NO APROBADA**. 2A no la modifica.

## 5. Roots Local / Central / Control

- Cada root tiene su propio `0001` y su propio historial SQLx.
- `DDL13-11` exige que los tres esquemas se instalen de forma aislada con los
  mismos números de migración.
- Hoy solo Local `0001` está aprobado y en `migrations/local`; Central y Control
  siguen siendo subconjuntos experimentales del Migration Lab.

Resolución técnica: mantener tres roots independientes, sin migraciones
compartidas por nombre y sin que Local dependa de Central/Control. Un `plan`
remoto no puede ser requisito para operar Free Local.

## 6. Qué puede ejecutarse sin aprobación nueva

| Acción | Estado |
| --- | --- |
| Dominio con costo opcional y prueba de «desconocido ≠ cero» | **Hecho (2A)** |
| Validación de cantidades, sobre-recepción y snapshots de partida | Ya existente en `pos-core` |
| Matriz de dependencias de migraciones candidatas | Hecho en 1Z |
| DDL de `unit_cost NULL` | **Autorizado (CC21-03, 2026-10-30)**; la propuesta `0016` lo implementa sin promoverla |
| Fichas de los cinco padres | **Bloqueado** por decisión |
| Import sintético versionado | **Bloqueado** por DDL aprobado |

## 7. Decisión registrada — `CC21-03`

**Aceptada por el titular el 2026-10-30** (documento `00B · Control maestro de
revisiones y aprobaciones — HiQNet POS`).

Pregunta resuelta:

> ¿Se acepta que el destino físico represente el costo pactado como `NULL`
> (opción A), que el total de orden sea `NULL` cuando falte algún costo y que los
> reportes muestren «sin costo» en lugar de cero?

**Sí.** Alcance aceptado:

1. `unit_cost TEXT NULL`; `NULL` es costo pactado desconocido y `0.0000` es un
   cero conocido.
2. Los valores conocidos son decimales exactos; prohibido el punto flotante.
3. El total derivado es `NULL` cuando alguna partida carece de costo pactado; no
   se publica un subtotal parcial como total confirmado.
4. Los reportes distinguen «sin costo» de cero.
5. No se inventan costos históricos ni se infiere el costo efectivo de recepción
   a partir del costo pactado.

Lo que la aceptación **no** cubre y sigue pendiente en `ADR-0021`: las cinco
fichas físicas ausentes, costeo efectivo de recepción, valoración histórica,
regularización económica, snapshots pendientes, políticas de recepción nueva e
import y reconciliación de datos históricos.

Registro: este documento **no** modifica `ADR-0021`, que permanece `PROPOSED`.
