# ADR-0021: conflictos del físico congelado y paridad Java→Rust

## Estado

**PROPUESTO SIN APROBACIÓN.** Fecha: 2026-10-27.

No hay alternativa de diseño aceptada, aprobador ni autorización para modificar
el freeze. Este registro no sustituye F4–21, F1–10, F1–12 ni ADR-0020; tampoco
cambia migraciones, contratos HTTP o autoridad productiva.

Baseline Java: `ebf7a30afc342a31d837a85772c5e85d0bc38efb`.
Checkout de trabajo: `feat/backend/rust-foundation-migration-lab`.
La [auditoría física](../../05-development/rust-physical-audit.md) conserva el
índice de nombres, las referencias por sección y la evidencia de código/líneas,
sin copiar el expediente confidencial.

## Contexto y hechos comprobados

| ID | Divergencia | Impacto |
| --- | --- | --- |
| PHY-PAR-01 | F1–10 declara 83 tablas, pero contiene 82 definiciones únicas: 59 Local, 9 Central, 14 Control Plane. El HTML incluye un elemento de tabla vacío. | No se puede deducir ni inventar una entidad número 83. Corregir el conteo no equivale a agregar una tabla. |
| PHY-PAR-02 | `price_book`, `tax_profile`, `CostingPolicy`, `ExpenseCategory` y `plan` son dependencias sin definición física tabular en F1–10. Los dos nombres PascalCase no fijan aquí un nombre SQL de destino. | No se puede completar el esquema creando estructuras mínimas supuestas ni omitiendo FKs. |
| PHY-PAR-03 | Java admite `unit_cost = NULL`, incluso tras emisión y recepción. F1–12 exige preservarlo nullable; F1–10 §16.40 declara `unit_cost_exact` y `line_total_exact` no nulos. | No hay una conversión completa sin pérdida que cumpla ambos contratos actuales para esos registros. |
| PHY-PAR-04 | Java V25 no guarda costo propio de recepción; F1–10 §16.41 exige `unit_cost_exact` no nulo. | Falta una regla aprobada de origen/valoración; copiar el costo actual o inventar cero no preserva el hecho histórico. |
| PHY-PAR-05 | F1–12 exige snapshots de compras, pero las fichas §16.16/§16.40 no enumeran destinos suficientes para moneda/snapshots de proveedor y unidad/código/nombre de partida. | Falta cerrar el mapping físico; no reconstruir snapshots desde el catálogo actual. |

DRAFT limita la **edición** de partidas, no la persistencia de costo desconocido.
La evidencia incluye [V23:63–84](../../../backend/src/main/resources/db/migration/V23__create_purchasing_purchase_order_foundation.sql#L63),
[emisión:397–423](../../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseOrderService.java#L397)
y el [fixture sin costo:68–102](../../../backend/src/test/java/com/hiqnet/pos/purchasing/api/PurchaseReceiptApiTest.java#L68)
que alimenta el [caso RECEIVED:151–195](../../../backend/src/test/java/com/hiqnet/pos/purchasing/api/PurchaseReceiptApiTest.java#L151).
Son fuentes inspeccionadas; esta revisión no ejecutó esas pruebas.

## Default operativo mientras no exista aprobación

**Detener la importación afectada y mantener el bloqueo material de full completion
0b.** Es una medida conservadora solicitada, no la aprobación de una alternativa
semántica ni la implementación de un guard en código.

- Preservar `NULL` y snapshots originales en source fixtures y bundles versionados.
  `NULL` no es cero; no convertirlo a `0`, `0.0000`, costo de catálogo o promedio.
- Un ensayo de importación afectado debe fallar explícitamente antes de aplicar
  cambios o revertir su transacción completa; no omitir filas y declarar paridad.
- No cambiar nulabilidad, estados, campos, FKs ni políticas para hacer pasar gates.
- No declarar compras conformes ni DDL-13/PAR/AS completos con esas brechas abiertas.
- Puede continuar el resto del laboratorio: fixtures Java, exportación fiel,
  checksums, failure/restore y subconjuntos documentados independientes. Esa
  continuidad no autoriza merge de esquema completo, import real ni cutover.

## Alternativas propuestas, no aprobadas

| Alternativa | Beneficio | Coste, riesgo y control requerido |
| --- | --- | --- |
| A. Conservar contratos actuales y detener import afectado (default temporal) | Preserva datos y no resuelve unilateralmente el freeze. | No cierra paridad de compras ni full completion 0b. No es solución final de migración. |
| B. Alinear el destino con costo desconocido legado | Permitiría conservar semántica nullable sin inventar valor. | Definir nulabilidad/semántica del total y costo de recepción, valoración y mapping de snapshots. Cambios de invariantes requieren ADR/change control y actualización documental; no basta volver nullable una columna. |
| C. Exigir costo antes de emitir o recibir en el nuevo dominio | Puede simplificar operaciones futuras con costo completo. | Cambia comportamiento Java: ORDERED/RECEIVED sin costo existen como casos válidos. Requiere aprobación y estrategia histórica sin pérdida; limitar NULL a DRAFT no resuelve la migración. |
| D. Diseñar representación histórica diferenciada y trazable | Podría separar hechos legados incompletos de operaciones futuras valoradas. | Requiere contrato explícito de representación, consumo y reconciliación. No autoriza nuevas tablas/campos ni una solución JSON ad hoc por este ADR. |

Ninguna alternativa autoriza inferir las cinco entidades físicas faltantes.
Tampoco se adopta una política de valoración provisional por el mero hecho de que
F4–13 contemple escenarios provisionales: falta resolver el caso concreto y su
representación física.

## Criterios para resolver y revisar el estado

1. Owner y responsables de arquitectura/dominio/datos registran alternativa,
   alcance, compatibilidad y aprobación explícita. Este documento no la atribuye.
2. Reconciliar F1–10/F1–12 con F4–21; cambios de invariantes pasan por change
   control. Una corrección puramente editorial del conteo se distingue de DDL nuevo.
3. Definir las dependencias físicas, destinos de snapshots y crosswalk de nombres
   antes de materializarlos; no crear ambas variantes de un mismo nombre lógico.
4. Aportar pruebas que distingan NULL/cero/costo informado en DRAFT, ORDERED,
   PARTIALLY_RECEIVED y RECEIVED; comparar snapshots, UUIDs y cantidades históricos.
5. Ejecutar los gates DDL-13, PAR-01..09 y AS-01..10 aplicables con evidencia real.
   Resolver este ADR por sí solo no demuestra full completion 0b.

## Fuentes normativas exactas

Consultadas mediante TXT el 2026-10-27; son referencias, no copias del expediente.

- [F1–10 §16.16, §16.18–19, §16.37–41, §16.60 y §17](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/export?format=txt): físico y brechas.
- [F1–12 §2 y §4](https://docs.google.com/document/d/16hyT4XALY3DrAc50RUTx-fESzMI0c5612zTKphHRieM/export?format=txt): preservación y conversiones exactas.
- [F1–11 §7 y ratificación final](https://docs.google.com/document/d/1PCgbE95Qt3GjIfy78rPt-tCMjQ2Ed2GwBzEcTfa4HMU/export?format=txt): gates de implementación.
- [F4–21 §5–7](https://docs.google.com/document/d/1tIHbbRw5GhKOFkL4ff0Zoc-SmMc-q7J2WwyYXFjqiUE/export?format=txt): límites del freeze y change control.
- [F1–13 §4–6](https://docs.google.com/document/d/1ZtDnxOeVjeBJD20gLI1GxA4cTY-bevRoL2fKUMqZBPk/export?format=txt): escenarios y assertions, no evidencia de ejecución.
