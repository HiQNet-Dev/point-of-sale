# Auditoría física Rust — conteo y conflictos de paridad

**Fecha:** 2026-10-27. **Estado:** hechos inspeccionados; resolución de diseño
**PROPUESTA SIN APROBACIÓN** en [ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md).
**Full completion 0b: BLOQUEADO materialmente por las brechas de este informe.**
El resto del laboratorio puede continuar sin darlas por resueltas.

Baseline Java: `ebf7a30afc342a31d837a85772c5e85d0bc38efb`; rama de trabajo
`feat/backend/rust-foundation-migration-lab`, con foundation local preexistente.
Este documento registra solo índice de nombres, referencias y hechos necesarios;
no reproduce campos completos, diagramas ni el expediente confidencial.
No modifica el freeze, Java/Flyway, Rust/SQLx, OpenAPI ni gates ejecutables.

## 1. Fuentes y autoridad

Los cuatro documentos principales se consultaron por `export?format=txt`.
F4–21 ratifica el diseño Free Local y prevalece sobre párrafos históricos que aún
se declaran propuestas abiertas; F1–10 gobierna el físico, F1–11 los gates y F1–12
la paridad. Una contradicción entre ellos se registra, no se resuelve por elección
silenciosa. Las URLs son documentos mutables, no revisiones inmutables de Drive.

| Referencia exacta | Evidencia usada |
| --- | --- |
| [F1–10](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/export?format=txt) | §16–17: nombres y fichas físicas; nulabilidad y dependencias. |
| [F1–12](https://docs.google.com/document/d/16hyT4XALY3DrAc50RUTx-fESzMI0c5612zTKphHRieM/export?format=txt) | §2: mapping 37/37; costo nullable, unidad y supplier snapshots. §4: exactitud. |
| [F1–11](https://docs.google.com/document/d/1PCgbE95Qt3GjIfy78rPt-tCMjQ2Ed2GwBzEcTfa4HMU/export?format=txt) | §7 y ratificación: implementación/release no equivalen a design freeze. |
| [F4–21](https://docs.google.com/document/d/1tIHbbRw5GhKOFkL4ff0Zoc-SmMc-q7J2WwyYXFjqiUE/export?format=txt) | §5–7: gates, fuentes canónicas y cambios sujetos a ADR. |
| [F1–13](https://docs.google.com/document/d/1ZtDnxOeVjeBJD20gLI1GxA4cTY-bevRoL2fKUMqZBPk/export?format=txt) | §4–6: DDL13-01..11, PAR-01..09 y AS-01..10. |
| [F4–15](https://docs.google.com/document/d/12VYpuy-IznZufJpYiGqn-4BBi3JqeI0al3XCE5Gx7tU/export?format=txt) | §4–6: transición de OC y scopes explícitos, ratificados por F4–21. |
| [F4–13](https://docs.google.com/document/d/1vL-yFrwenxeqbAjuWY9m5zOMhlMGADfVTDUeWaOa4OM/export?format=txt) | §7–10 y ratificación: separación cantidad/valor, costo y provisionalidad; no regla NULL→0. |
| [F1–09](https://docs.google.com/document/d/1aPZZcWmf3GvoNFfsLnpESg_Rym9V49-1DcTMwoxsvnw/export?format=txt) | §1, §11 y referencia tabular: entidades lógicas no equivalen a tablas físicas; no crear opcionales por mención. |
| [Matriz complementaria F1–10](https://docs.google.com/spreadsheets/d/1ruR5jU38WZFwwv_tzQay2Y28CqLYOfpbYSU7m4hyHD4/export?format=csv) | Hoja exportada de resumen: 87 entidades rastreadas; no sustituye las fichas físicas. No se afirma auditar todas las pestañas. |

## 2. Conteo reproducible: 82, no 83 definiciones

Sobre el TXT se identificaron encabezados con el patrón
`(?m)^[ \t]*((?:16|17)\.\d+) ([a-z][a-z0-9_]*)`, comprobando un bloque de
campos por definición y nombres únicos. El espacio/tab inicial importa: algunos
encabezados vienen precedidos por tabulación.

| Control | Resultado |
| --- | --- |
| §16.1–§16.67 | 67 definiciones, sin huecos. |
| §17.1–§17.15 | 15 definiciones, sin huecos. |
| Nombres únicos / encabezados `Campo` del TXT | 82 / 82. |
| POS Local / POS Central / Control Plane | **59 / 9 / 14**. |
| HTML exportado | 83 elementos `table`, uno vacío después del bloque `cash_register`; 82 con contenido de definición. |

El [HTML contrastado](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/export?format=html)
permite distinguir objetos de formato de definiciones de BD. La tabla vacía
**podría** explicar el 83 editorial; no se atribuye esa causa como hecho probado.
No hay una entidad número 83 identificable por diferencia aritmética.

SHA-256 del TXT F1–10 contado, estable en la reconsulta de esta fecha:

```text
8a3354af4c1634ad0c97ebccb8be787202e67423d37cc5df548d4137e4549e5f
```

### Índice POS Local — 59

Los nombres de cada fila siguen el orden de las secciones; no son DDL ni fichas
campo por campo.

| Secciones F1–10 | Nombres físicos |
| --- | --- |
| §16.1–6 | `organization`, `branch`, `installation_identity`, `pos_user`, `local_credential`, `branch_membership` |
| §16.7–12 | `cash_register`, `cash_shift`, `sale`, `sale_line`, `payment_record`, `cash_movement` |
| §16.13–20 | `inventory_item`, `branch_stock`, `inventory_movement`, `purchase_order`, `purchase_receipt`, `valuation_entry`, `expense`, `expense_payment` |
| §16.21–24 | `domain_event`, `outbox_delivery`, `command_dedup`, `sync_checkpoint` |
| §16.33–39 | `return_record`, `return_line`, `refund_record`, `cash_count`, `price_version`, `tax_rule_version`, `sale_line_tax_snapshot` |
| §16.40–44 | `purchase_order_line`, `purchase_receipt_line`, `waste_record`, `stock_count`, `stock_count_line` |
| §16.45–52 | `local_session`, `role`, `permission`, `role_permission`, `membership_role`, `license_snapshot`, `backup_manifest`, `restore_checkpoint` |
| §17.1–8 | `organization_role_assignment`, `audit_event`, `bootstrap_record`, `category`, `product`, `modifier_group`, `modifier_option`, `product_modifier_group` |
| §17.9–15 | `sale_line_modifier_snapshot`, `product_inventory_recipe`, `product_inventory_recipe_component`, `modifier_option_inventory_config`, `modifier_option_inventory_effect`, `sale_line_inventory_component`, `supplier` |

### Índice POS Central — 9

| Secciones F1–10 | Nombres físicos |
| --- | --- |
| §16.25–27 | `operational_tenant`, `received_event`, `sync_cursor` |
| §16.53–58 | `central_branch`, `authorized_installation`, `applied_operation`, `stock_projection`, `consolidated_sale`, `sync_incident` |

### Índice Control Plane — 14

| Secciones F1–10 | Nombres físicos |
| --- | --- |
| §16.28–32 | `customer`, `subscription`, `billing_event`, `installation_registration`, `license` |
| §16.59–67 | `business_registration`, `plan_version`, `entitlement_definition`, `plan_entitlement`, `license_activation`, `support_consent`, `control_audit_event`, `provider_webhook_receipt`, `subscription_provider_binding` |

## 3. Cinco dependencias sin definición física tabular

| Contexto | Dependencia | Evidencia F1–10 | Brecha |
| --- | --- | --- | --- |
| Local | `price_book` | §16.37: `price_book_id`, FK no nula, referencia `price_book(id)`. | No hay ficha física de la tabla destino. |
| Local | `tax_profile` | §16.38: `tax_profile_id`, FK no nula, referencia `tax_profile(id)`. | No hay ficha física de la tabla destino. |
| Local | `CostingPolicy` | §16.18: `costing_policy_id`, UUID, FK no nula, política versionada. | No hay ficha ni nombre SQL de destino explícito en esa referencia. |
| Local | `ExpenseCategory` | §16.19: `category_id`, UUID, FK no nula, observación `ExpenseCategory`. | No hay ficha ni nombre SQL de destino explícito en esa referencia. |
| Control Plane | `plan` | §16.60: `plan_id`, FK no nula, referencia `plan(id)`. | No hay ficha física de la tabla destino. |

No son cinco tablas autorizadas para inventar, ni identifican una supuesta tabla
83. Las 87 entidades lógicas del anexo no constituyen 87 fichas físicas. No crear
estructuras con solo `id`, quitar FKs o completar campos por intuición.

## 4. Evidencia `unit_cost`: ORDERED y RECEIVED admiten NULL

Rutas relativas al repositorio; las líneas enlazadas corresponden al baseline.
Las pruebas citadas se inspeccionaron como fuente, **no se ejecutaron en esta
auditoría**. No se declara un resultado runtime nuevo.

| Capa / estado | Evidencia de código y líneas | Hecho comprobado |
| --- | --- | --- |
| SQL | [V23:63–84](../../backend/src/main/resources/db/migration/V23__create_purchasing_purchase_order_foundation.sql#L63), en particular L73; [comentario:109–110](../../backend/src/main/resources/db/migration/V23__create_purchasing_purchase_order_foundation.sql#L109) | `unit_cost NUMERIC(19,4)` nullable, CHECK NULL o >=0, sin condición de estado. Es costo pactado opcional, no valoración. |
| Entidad | [PurchaseOrderLine:83–84](../../backend/src/main/java/com/hiqnet/pos/purchasing/domain/PurchaseOrderLine.java#L83), [218–230](../../backend/src/main/java/com/hiqnet/pos/purchasing/domain/PurchaseOrderLine.java#L218) | La columna no exige no-nulidad; el validador devuelve NULL cuando recibe NULL. Cero es un valor diferente. |
| DTO / PATCH | [Create:24–29](../../backend/src/main/java/com/hiqnet/pos/purchasing/api/CreatePurchaseOrderLineRequest.java#L24), [Update:26–32](../../backend/src/main/java/com/hiqnet/pos/purchasing/api/UpdatePurchaseOrderLineRequest.java#L26), [controller:277–287](../../backend/src/main/java/com/hiqnet/pos/purchasing/api/PurchaseOrderController.java#L277) | Alta opcional; PATCH distingue omitir, limpiar con NULL y asignar cero. |
| Exactitud | [PurchaseOrderAmounts:53–71](../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseOrderAmounts.java#L53), [90–91](../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseOrderAmounts.java#L90) | NULL se conserva. Valor informado usa string decimal no negativo, escala <=4 y rango NUMERIC(19,4); no float. |
| Solo edición DRAFT | [PurchaseOrderService:249–263](../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseOrderService.java#L249), [297–309](../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseOrderService.java#L297) | `requireDraft()` limita la mutación; no exige que una partida deje de ser nullable al emitir. |
| Emisión ORDERED | [PurchaseOrderService:397–423](../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseOrderService.java#L397), [506–534](../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseOrderService.java#L506), [PurchaseOrder:233–247](../../backend/src/main/java/com/hiqnet/pos/purchasing/domain/PurchaseOrder.java#L233) | Exige líneas, proveedor/artículos activos y unidad compatible, no costo. La transición no rellena el costo. |
| Caso ORDERED sin costo | [PurchaseOrderTransitionApiTest:75–81](../../backend/src/test/java/com/hiqnet/pos/purchasing/api/PurchaseOrderTransitionApiTest.java#L75), [112–136](../../backend/src/test/java/com/hiqnet/pos/purchasing/api/PurchaseOrderTransitionApiTest.java#L112) | Helper omite `unitCost`; el test espera emisión exitosa a ORDERED. |
| Recepción | [PurchaseReceiptService:142–188](../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseReceiptService.java#L142), [449–467](../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseReceiptService.java#L449) | Recibe desde ORDERED/PARTIALLY_RECEIVED; valida cantidades y scope, sin exigir ni completar costo. |
| Caso RECEIVED sin costo | [PurchaseReceiptApiTest:68–102](../../backend/src/test/java/com/hiqnet/pos/purchasing/api/PurchaseReceiptApiTest.java#L68), [151–195](../../backend/src/test/java/com/hiqnet/pos/purchasing/api/PurchaseReceiptApiTest.java#L151) | Fixture crea sin costo y emite; el test espera recepción completa, RECEIVED y stock ingresado. |
| Distinción NULL / cero | [PurchaseOrderApiTest:258–295](../../backend/src/test/java/com/hiqnet/pos/purchasing/api/PurchaseOrderApiTest.java#L258) | Comprueba conservar, limpiar a SQL NULL y asignar `0.0000` por separado. |
| Evolución SQL / receipt | [V25:53–78](../../backend/src/main/resources/db/migration/V25__create_purchasing_purchase_receipts.sql#L53), [132–155](../../backend/src/main/resources/db/migration/V25__create_purchasing_purchase_receipts.sql#L132), [PurchaseReceiptLine:39–61](../../backend/src/main/java/com/hiqnet/pos/purchasing/domain/PurchaseReceiptLine.java#L39) | V25 habilita estados recibidos sin volver obligatorio el costo de PO; receipt line no tiene costo propio. |

La frase histórica de V23 «solo DRAFT» describe el incremento original, no una
restricción actual de nulabilidad. F4–15 §4 tampoco establece costo obligatorio
para emitir: indica líneas válidas, supplier/items activos y fin de edición DRAFT.

### Contradicción con el físico congelado

| Fuente | Contrato relevante |
| --- | --- |
| F1–12 §2, fila `purchase_order_lines` | PRESERVAR: cantidad exacta, `unit_cost` exacto nullable y unidad snapshot. |
| F1–10 §16.40 | `unit_cost_exact` y `line_total_exact`: NULL = NO, sin default. |
| F1–10 §16.41 | `unit_cost_exact` de recepción: NULL = NO para valoración de entrada. |

Costo desconocido no es costo cero. Sin costo no puede calcularse un total
verdadero; volver nullable solo `unit_cost_exact` no resuelve el total ni el
costo requerido de recepción. Java no aporta un costo histórico independiente de
receipt. No se presume que costo pactado, costo efectivo y promedio sean iguales.

## 5. Snapshots y destinos de paridad incompletos

Ausencia en una ficha no prueba pérdida ya ejecutada: **no hay aquí una
importación evaluada**. Señala falta de destino/mapping explícito suficiente.

| Hecho legado / exigencia | Evidencia | Brecha física a resolver |
| --- | --- | --- |
| Moneda y snapshots de proveedor de la cabecera | [V23:17–25](../../backend/src/main/resources/db/migration/V23__create_purchasing_purchase_order_foundation.sql#L17); F1–12 §2, `purchase_orders`: currency/supplier snapshot. | F1–10 §16.16 no enumera `currency`, `supplier_code_snapshot`, `supplier_name_snapshot`; tampoco `code` ni los actores legados. No asumir reconstrucción desde supplier actual. |
| Código, nombre y unidad históricos de partida | [V23:68–75](../../backend/src/main/resources/db/migration/V23__create_purchasing_purchase_order_foundation.sql#L68); [PurchaseOrderLine:67–78](../../backend/src/main/java/com/hiqnet/pos/purchasing/domain/PurchaseOrderLine.java#L67); F1–12 §2 exige unidad snapshot. | §16.40 no enumera `item_code_snapshot`, `item_name_snapshot`, `unit_snapshot`, `line_number` ni timestamps de línea. `item_id` actual no sustituye esos snapshots. |
| Scope explícito | F4–15 §5.1/§6; F1–10 §16.7/§16.8. | CashShift referencia CashRegister con organización/sucursal, pero §16.7 no enumera `organization_id`. F4–15 sí aporta ese campo/clave: reconciliar las fichas, no inventar otra estrategia. |
| Crosswalk de nombres | F1–12 §2 frente a F1–10 §17.11–13. | `recipe_component` / `modifier_inventory_config` / `modifier_inventory_effect` frente a `product_inventory_recipe_component` / `modifier_option_inventory_config` / `modifier_option_inventory_effect`. No son autorización para duplicar entidades. |

## 6. Impacto sobre 0b y trabajo que puede continuar

El bloqueo es de **full completion 0b y paridad afectada**, no una revocación del
design freeze ni una prohibición general de trabajar en el laboratorio.

- Mantener source Java/Flyway intacto; preservar NULL, UUIDs y snapshots en
  fixtures/bundles. Un export fiel no demuestra que el import destino sea viable.
- Default: detener import afectado antes de efectos parciales o revertir toda su
  transacción. Es una pauta documental; no se implementó un guard en este cambio.
- No dar por verdes compras/paridad, esquema completo ni DDL-13/PAR/AS mientras
  las divergencias materiales y su evidencia sigan abiertas.
- Puede avanzar harness técnico, exportación fiel, failure/restore y DDL de
  subconjuntos con dependencias documentadas. Ejemplos: `category`→`product`,
  modifiers/asociación y `supplier` sobre `organization`, respetando F4–15.
- No inventar la tabla 83, las cinco dependencias, defaults, snapshots ni valores
  de costo para cerrar un clean install. Tampoco quitar FKs por conveniencia.
- [ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md)
  expone alternativas sin elegir una. Cambiar nulabilidad, reglas de emisión,
  valoración o representación histórica exige aprobación/change control según
  su impacto; este informe no actualiza F4–21.

## 7. Alcance de la validación

La investigación comprobó 25 migraciones Flyway y 37 tablas AS-IS, con bytes
idénticos al baseline; contrastó las entidades, servicios y pruebas principales
citados. Recontó los TXT y examinó el HTML; no descargó copias del expediente al
repositorio. No ejecutó pruebas Java/Rust, migraciones, importaciones ni cutover.
Los casos de prueba citados son evidencia fuente, no nuevos resultados de CI.
Este informe tampoco certifica el estado actual de Docker, PostgreSQL o gates
que otras tareas del laboratorio puedan haber ejecutado.
