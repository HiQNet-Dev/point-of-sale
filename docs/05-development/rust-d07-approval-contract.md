# D07 — contrato propuesto final de snapshots, procedencia y auditoría

**PROPOSED, NO ADOPTADO · 2026-10-28 · base `329c098`.**
Completa el [mapping de compras 47/47](rust-purchase-parity-contract.md) §3–4;
no lo sustituye por una reconstrucción. Las columnas originales se conservan,
con los deltas P siguientes para revisión conjunta en ADR-0021/CC-0021.
Ningún destino añadido está autorizado como DDL.

## 1. Cierre del mapping de las 47 columnas

| Origen | Campos cubiertos | Regla final recomendada P |
| --- | ---: | --- |
| `purchase_orders` | 17/17 | Conservar UUID, org/branch/supplier, code, status, currency, ambos snapshots supplier, tres actores, version y cuatro instantes. Los destinos ausentes enumerados como P se agregan como columnas, no se sustituyen por JOIN actuales. |
| `purchase_order_lines` | 12/12 | Conservar UUID, org/orden/item, número de línea, código/nombre/unidad snapshot, cantidad exacta, costo nullable, creación/actualización. FK item con org; unicidades de item y número por orden preservadas. |
| `purchase_receipts` | 9/9 | Conservar UUID, org/branch/orden, clave textual y fingerprint Java, receptor, received_at y created_at distintos. POSTED expresa recepción física, no valor confirmado. |
| `purchase_receipt_lines` | 9/9 | Conservar UUID, org/branch/receipt/orden/partida/item, cantidad ingresada y created_at. Validar igualdad de item con partida y movimiento, además de FK de scope. |

Decimales fuente: strings exactos y escala fuente; fechas: UTC a microsegundo,
que coincide con TIMESTAMPTZ Java/PostgreSQL. Cualquier dato no representable
falla antes del commit, no se recorta. Textos/snapshots legados no se normalizan
con reglas de altas nuevas. Una colisión tras normalización se diagnostica, no
se resuelve renombrando historia.

**Delta concreto de nombre:** recomendar `purchase_order.version` para la revisión
persistida, conservando su valor Java no negativo. Cambiar el candidato físico
`expected_version` de F1–10 §16.16 a ese nombre; `expectedVersion` sigue siendo
precondición de comando/DTO, no un segundo contador ni valor para resetear al importar.
La tabla §3.1 de 0c documenta el nombre congelado previo; este delta P lo reemplaza
solo en la versión de aprobación. REST no cambia en esta intervención.

**Precisión/representación:** SQLite STRICT UUID TEXT canónico, instantes TEXT UTC
fijo con seis decimales y decimales TEXT; PostgreSQL UUID, TIMESTAMPTZ(6), cantidad
NUMERIC(19,6), costo NUMERIC(19,4), total pactado NUMERIC(38,10). Validación antes de
INSERT en ambos. Nuevos valores económicos usan el [anexo DEC1](rust-valuation-approval-contract.md),
no se recalculan históricos para adaptar su escala o moneda.

## 2. Hechos ausentes: representación, no invención

| Hecho que Java no almacenó | Contrato recomendado P |
| --- | --- |
| Costo efectivo de receipt/valoración/COGS | Campo económico nullable, estado inicial AT_IMPORT/PENDING y procedencia; cero pactado no es evidencia de cero efectivo. Sin ValuationEntry ficticio. |
| Cantidad rechazada o total entregado antes de rechazo | `rejected_quantity_exact` nullable para legado, acompañado de `rejection_evidence_state=UNRECORDED_LEGACY`; no default cero. Para operación nueva registrar explícitamente la decisión, incluso cero conocido; escala (19,6), CHECK >=0. |
| Motivo de cancelación | Conservar ausencia nullable para legado y procedencia; no rellenar «migrado». Nuevos comandos requieren motivo según F4–15. |
| Cierre parcial/motivo de remanente | `closed_at`/`close_reason` NULL donde no existían; no convertir PARTIALLY_RECEIVED/CANCELLED a CLOSED_PARTIAL. |
| Impuestos/flete/descuento/factura/pago propios de receipt | Ausencia explícita en envelope de origen; no derivar desde supplier/product actuales ni interpretar receipt como factura pagada. |
| Evento de dominio/outbox, writer generation y secuencia Rust | Hechos nuevos del import/baseline identificados como tales; no inventar comandos/eventos ejecutados por Java. `origin_event_id` nullable solo para legado con procedencia; nuevos movimientos conservan obligación de evento atómico. |
| Estado de valoración observado en fecha histórica | Clasificar hoy con AT_IMPORT y assessed_at actual; no afirmar que Java produjo PENDING/PROVISIONAL/CONFIRMED. |

Proponer `rejection_evidence_state` TEXT NN (UNRECORDED_LEGACY/DOCUMENTED), sin
valor por defecto que afirme conocimiento; nuevo DOCUMENTED exige cantidad NN.
Los otros datos ausentes se enumeran en `unrecorded_fields` de un envelope de
proveniencia versionado, **no** se agregan columnas de negocio inventadas para
simularlos. Envelope no reemplaza UUID/FK, estados, importes ni snapshots existentes.
No se afirma ausencia de rechazo real: únicamente ausencia de registro en origen.

## 3. Inventario y correlación: completar las relaciones

- Receipt line → InventoryMovement: resolver por
  `inventory_movements.purchase_receipt_line_id`, tipo PURCHASE_RECEIPT y mismo
  org/branch/item. Debe existir exactamente uno para una línea publicada íntegra;
  ausencia/multiplicidad falla import, no se fabrica un movimiento.
- Preservar `inventory_movements.actor_user_id`, `resulting_quantity`, `reason`
  y referencias `sale_id`/`purchase_receipt_line_id` en columnas scoped del destino,
  además de id/org/branch/item/tipo/delta/fecha. Añadir FK tipadas a Sale y
  PurchaseReceiptLine donde aplique; `source_type/source_id` solo no basta para
  acreditar integridad. Conservar CHECK de alternativas excluyentes de V25.
- `purchase_receipt_line.movement_id` conserva referencia inversa exacta. Para
  bootstrap evitar ciclo NOT NULL imposible: insertar líneas aún sin vínculo
  inverso y movimientos en una TX, enlazar **antes de publicar** mediante etapa
  de import validada; tras publicación todo append-only. La implementación deberá
  escoger FK diferibles soportadas y prueba por motor, no desactivar foreign_keys
  ni permitir UPDATE de un hecho ya publicado. Alternativa preferida más simple:
  **eliminar la columna inversa redundante del físico**, resolver por FK única
  movimiento→línea y vista/query; esta es la opción seleccionada P para 0d y exige
  actualizar F1–10 §16.41. No se necesitan ambos vínculos persistidos.
- Importar filas/movimientos/balances preservados no llama al caso de uso receipt:
  volver a ejecutarlo duplicaría stock. Validar delta=cantidad, sumas por partida,
  estados y resulting_quantity, no solo row counts o FK.
- Secuencias nuevas no renumeran movimientos históricos. Orden causal único,
  ambiguo o inconsistente se trata como en DEC1 §7; export ordenado no acredita MWAC.

## 4. Auditoría: mapping adicional 12/12, no eventos regenerados

Fuente inspeccionada: [V7](../../backend/src/main/resources/db/migration/V7__create_audit_events.sql)
L14–48 y [AuditEvent](../../backend/src/main/java/com/hiqnet/pos/audit/domain/AuditEvent.java)
campos L37–73; físico consultado F1–10 §17.2. Este inventario es **adicional**, no
infla las 47 columnas de compras ni demuestra PAR-01 completo.

| Columna Java | Destino propuesto / conservación |
| --- | --- |
| `id` | `audit_event.id`, mismo UUID. |
| `organization_id` | Homónimo, FK Organization. |
| `branch_id` | Homónimo nullable, FK(org,branch) cuando exista. |
| `actor_type` | Homónimo USER/SYSTEM; SYSTEM no recibe actor sintético. |
| `actor_user_id` | Homónimo nullable, USER exige FK(org,user); SYSTEM exige NULL. |
| `token_jti` | Nuevo P `legacy_token_jti` TEXT nullable, máximo 64; correlador forense, **no JWT ni credencial**. No reinterpretarlo como session_id ni habilitar sesión. |
| `action` | Homónimo, texto histórico exacto, máximo fuente 80. |
| `entity_type` | Homónimo, texto histórico exacto, máximo fuente 40. |
| `entity_id` | Homónimo nullable; UUID fuente conservado, no retarget a entidad nueva por coincidencia. |
| `reason` | Homónimo nullable, texto máximo fuente 255. |
| `metadata` | `metadata_json`, JSON estructuralmente idéntico, canonicalización versionada de keys sin cambiar valores/arrays/NULL; conservar digest de representación de origen aparte. |
| `occurred_at` | Homónimo UTC microsegundo, no fecha de import. |

`audit_event.session_id` queda NULL para historia Java: F4–19 prohíbe importar
sesiones/tokens, y el JTI no es una sesión local. V7 documenta expresamente que
no guarda JWT, Authorization ni hash de token. Preservar ese identificador no
permite autenticación. No incluir JTI/metadata privados en reportes públicos.

Mantener límite de metadata 16 KiB del origen; un envelope de import se guarda
separado, no se mete dentro de metadata y cambia su contenido/límite. El destino
verifica tamaño en bytes de su formato canónico; si excede el límite, bloquear
unidad y reportar incompatibilidad, no eliminar claves silenciosamente.

Preservar eventos de compra como existen: `purchase_receipt.posted`, entity_type
`purchase_receipt`, entity_id receipt.id y metadata con purchaseOrderId,
purchaseReceiptId, branchId, lineCount. Correlación por esos campos cuando estén
presentes; no declarar una FK obligatoria retrospectiva a partir de cualquier
entity_id polimórfico. No fabricar un audit faltante para hacer cuadrar conteos.

La acción Rust de import es **otro evento nuevo**, actor/fecha reales del import,
con bundle/digest y resultado, no reemplazo ni replay de cada acción Java.
Conservación y nueva auditoría se cuentan por separado. UPDATE/DELETE de recibos,
movimientos y auditoría publicados deben ser rechazados por motor y aplicación.

## 5. Criterios de aceptación propuestos

1. Cobertura 47/47 y 12/12 audit sin duplicados; hashes por fila/UUID comparables;
   metadata JSON semánticamente igual y NULLs preservados.
2. Cambiar supplier/item/nombre/unidad/moneda actuales después del fixture no cambia
   snapshots importados; orden sin líneas conserva moneda/código propios.
3. Falta costo/rechazo/motivo/documento no produce cero/string por default; nueva
   operación valida su evidencia separadamente, nunca retrospectivamente.
4. Movimientos por receipt exactamente uno, cross-scope y delta discordante fallan;
   sin doble FK circular ni foreign_keys=OFF para completar import.
5. Reimport idéntico no-op; ID igual/contenido distinto conflicto atómico;
   fingerprint legacy conserva algoritmo UUID.compareTo y precisión 6. No aprobar
   como deseable el defecto de duplicados del replay Java descrito en 0c §7.2.
6. Historial de auditoría permanece append-only, conserva JTI solo forense;
   ninguna sesión creada, secreto registrado o evento histórico regenerado.
7. Inyectar fallo tras cualquier fase revierte filas/balance/audit/import identity;
   backup/restore conserva esos digests. Ejecutar PAR-01/04/06/07/09 y AS-01..10
   correspondientes después del DDL aprobado; este documento no los aprueba.

Cambios canónicos solicitados: F1–10 §16.15–17/40–41 y §17.2, F1–12 §2/4,
F1–11 scopes/append-only/snapshots, F4–15 §4–6 y referencia F4–21 §1/5–7.
Estos deltas, incluidos nullable de rechazo y retiro del vínculo inverso, necesitan
aceptación explícita; el freeze vigente no se edita aquí.
