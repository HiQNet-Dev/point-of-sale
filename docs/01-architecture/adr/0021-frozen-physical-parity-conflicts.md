# ADR-0021: conflictos del físico congelado y paridad Java→Rust

## Estado y control

**PROPOSED / SIN ACEPTACIÓN FORMAL.** Creado: 2026-10-27.
Revisión de aprobación **PRE0D-1: 2026-10-28**. **La dirección funcional D01..D07
fue aprobada con condiciones por el titular el 2026-10-28**, pero el ADR **no** está
ACCEPTED: aprobador por decisión, revisores, fecha de cierre y baseline canónico
nuevo siguen pendientes. No se infiere aceptación de la orden de preparar 0d.

No modifica F4–21, los documentos canónicos, migraciones, API ni autoridad
productiva. La recomendación siguiente no equivale a aceptación. F4–21 sigue
siendo la autoridad; Java conserva la operación real.

- Baseline Java: `ebf7a30afc342a31d837a85772c5e85d0bc38efb`.
- Base de integración observada: `origin/dev@6a15ee6a21ef80c17ab7276b337946cbcc4a4d37`.
- Rama 0c: `feat/backend/rust-physical-resolution`.
- [Reconciliación Git](../../05-development/rust-increment-0c-git.md): los seis
  commits anteriores ya están en `dev`, con CI roja. No volver a integrarlos.

## Versión sometida a aprobación PRE0D-1

Base reconciliada: `origin/dev@329c0984722e8339f3e3b29497960d426ac2c43c`;
los dos commits 0c ya están publicados. Rama nueva `feat/backend/rust-0d-readiness`.
El [paquete ejecutivo](../../05-development/rust-0d-approval-package.md) contiene
matriz D01..D07, alternativas, impactos, estado remoto/CI y el registro de la
dirección aprobada con condiciones.

Contratos finales **propuestos**, que concretan o reemplazan candidatos parciales
0c sin cambiar el freeze vigente:

- D01: corrección editorial a 82 fichas del inventario comprobado, no entidad 83.
- D02: [21 respuestas de catálogos](../../05-development/rust-d02-approval-contract.md),
  propiedad, lifecycle, FK/índices, asignación fiscal APPLY/STOP, refund acumulativo.
- D03..D06: [MWAC-DEC1](../../05-development/rust-valuation-approval-contract.md),
  evidencia local de receipt, continuidad física pending, escala18/HE/cota,
  causalidad y regularizaciones. Único contrato económico propuesto final.
- D07: [mapping y ausencias](../../05-development/rust-d07-approval-contract.md),
  47/47 compras y 12/12 audit, FK de ledger sin ciclo y sesiones Java no importadas.

Las tablas/resúmenes 0c de §3.1–3.2 siguientes son antecedentes parciales; los
anexos PRE0D-1 especifican los nombres/campos/estados finales propuestos. No hay
migración aprobada por publicar un anexo. Si no se acepta el paquete completo,
registrar alcance por D y mantener bloqueadas sus dependencias.

## 1. Contexto y evidencia

| ID         | Hecho contrastado                                                                                                                           | Consecuencia / resolución recomendada, no aceptada                                                                                                                                                           |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| PHY-PAR-01 | F1–10 declara 83 tablas; §16.1–67 y §17.1–15 contienen 82 fichas únicas: 59 Local, 9 Central, 14 Control. HTML tiene un objeto tabla vacío. | Tratar 82 como inventario comprobado; pedir al autor confirmación de errata o identificación de una entidad ausente. El vacío explica una hipótesis editorial, no prueba su causa. No inventar tabla 83.     |
| PHY-PAR-02 | Cinco destinos de FK carecen de ficha: `price_book`, `tax_profile`, `CostingPolicy`, `ExpenseCategory`, `plan`.                             | Revisar el diccionario propuesto y sus preguntas. El XLSX complementario menciona `costing_policy`/`expense_category`, pero eso no sustituye ficha ni aceptación. No crear padres solo con ID ni retirar FK. |
| PHY-PAR-03 | Java/F1–12 conservan `unit_cost` nullable incluso ORDERED/RECEIVED. F1–10 §16.40 exige costo y total NN.                                    | Preservar costo pactado desconocido y total desconocido juntos; distinguirlos de cero conocido. Cambiar ambas restricciones solo después de aprobación.                                                      |
| PHY-PAR-04 | Java V25 no tiene costo propio de recepción; F1–10 §16.41 lo requiere NN.                                                                   | Representar falta de evidencia histórica separadamente de valoración provisional o confirmada. No fabricar un asiento numérico para satisfacer el físico.                                                    |
| PHY-PAR-05 | F1–12 exige snapshots de compras; las fichas destino no enumeran todos sus campos, actores, tiempos y referencias.                          | Aprobar mapping exhaustivo antes del import; conservar hechos originales y declarar los que el origen nunca guardó. No reconstruir desde catálogo actual.                                                    |

La [auditoría física](../../05-development/rust-physical-audit.md) conserva las
referencias iniciales. La [reconsulta 0c y diccionario propuesto](../../05-development/rust-missing-physical-proposal.md)
registra secciones, huellas y negocio de las cinco dependencias. El
[contrato de compras](../../05-development/rust-purchase-parity-contract.md)
inventaría **47/47 columnas** de las cuatro tablas Java y el ledger relacionado.

Evidencia decisiva AS-IS:

- [V23](../../../backend/src/main/resources/db/migration/V23__create_purchasing_purchase_order_foundation.sql),
  `purchase_order_lines.unit_cost NUMERIC(19,4)` nullable, CHECK NULL o >=0.
- [PurchaseOrderService](../../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseOrderService.java),
  `issue`/`lockIssuableItems`: emisión no exige costo. DRAFT limita edición, no
  establece una condición de no-nulidad para ORDERED.
- [PurchaseReceiptService](../../../backend/src/main/java/com/hiqnet/pos/purchasing/application/PurchaseReceiptService.java),
  `post`/`applyStock`: recibe cantidades y crea stock/ledger sin costo efectivo.
- [V25](../../../backend/src/main/resources/db/migration/V25__create_purchasing_purchase_receipts.sql):
  receipt line no guarda costo propio; historial append-only y enlace de movimiento.
- Tests fuente `PurchaseOrderApiTest.updateLineCanKeepSetZeroAndClearUnitCost`,
  `PurchaseOrderTransitionApiTest.issueTransitionsDraftToOrderedStampsActorAndTimeAndAudits`
  y `PurchaseReceiptApiTest.fullReceiptPostsStockCompletesOrderAndAudits` distinguen
  NULL/cero y permiten emisión/recepción sin costo en sus fixtures.
  Su inspección no es una nueva ejecución local de Java.

## 2. Alternativas

| Opción                                                      | Contrato                                                                                                  | Ventajas                                                                        | Riesgos y decisión necesaria                                                                                                                                                        |
| ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A. Bloqueo conservador temporal                             | Export fiel; import afectado detenido antes de efectos o rollback completo.                               | No modifica freeze ni inventa datos.                                            | No cierra paridad. Sigue siendo el único comportamiento autorizado hoy.                                                                                                             |
| B. Alinear costo pactado nullable                           | Mantener desconocido en PO, total desconocido dependiente; valorar recepción solo con evidencia propia.   | Conserva semántica Java y distingue precio de adquisición/valor.                | Por sí sola no resuelve recepciones legadas ni stock ya consumido; exige completar representación y reconciliación.                                                                 |
| C. Costo obligatorio para toda historia y operación nueva   | Rechazar PO/receipt sin costo.                                                                            | Esquema homogéneo para datos completos.                                         | No representa registros válidos Java. Completar manualmente puede aportar evidencia nueva, pero no convierte esa evidencia en hecho histórico. No recomendada como regla de import. |
| D. Historia diferenciada con valoración pendiente explícita | Import conserva cantidades/snapshots; marca evidencia económica ausente; regularización nueva y trazable. | No confunde ausencia con cero/provisional; distingue legado de operación nueva. | Amplía el físico congelado y contrato de reportes; necesita control de cambios y política para consumo/valoración afectada. No basta una bolsa JSON.                                |
| E. Imputar cero, catálogo o promedio anterior               | Completar campos NN automáticamente.                                                                      | Permitiría insertar filas.                                                      | Destruye semántica y genera márgenes/valor ficticios. Descartada como recomendación; no autorizada por fuente alguna.                                                               |

## 3. Decisión recomendada (NO ADOPTADA): B + D, con frontera histórica

1. **Orden:** conservar costo pactado opcional para todos los estados históricos;
   recomendar mantenerlo opcional también al emitir nuevas PO. Una orden no
   certifica costo efectivo. Total pactado solo existe si el costo se conoce.
2. **Recepción legada:** importar el hecho físico una sola vez y su vínculo al
   movimiento original. Sin evidencia económica propia, conservar ausencia y
   procedencia. No copiar ni siquiera un costo pactado conocido como si fuera costo
   efectivo de recepción.
3. **Recepción nueva Rust:** recomendar exigir base de valoración documentada y
   contrato numérico/política versionada antes de confirmar. El costo pactado puede
   ser una base solo si se aprueba esa regla y se registra su procedencia; no es un
   fallback. Operar nuevas recepciones de cantidad sin costo queda fuera de esta
   recomendación v1 hasta decisión explícita. Esto es una diferencia de producto
   respecto de Java, no una validación a aplicar retrospectivamente.
4. **Valoración pendiente:** ausencia de evidencia NO es un asiento PROVISIONAL
   con cero. Proponer una marca en el hecho y un seguimiento auditable; ningún
   `valuation_entry` numérico hasta disponer de base válida. Una representación
   física auxiliar solo se agregará si se aprueba expresamente, no por este texto.
5. **Promedio móvil por Branch/Item:** mantener algoritmo congelado. Si Q y V están
   completamente valorados, Q>0 y entrada q con costo efectivo c documentado,
   `Q'=Q+q`, `V'=V+q*c`, `MWAC'=V'/Q'`, con contrato exacto versionado. Si falta
   valor en cualquier componente relevante, no emitir un promedio confirmado.
   No dividir el valor conocido entre cantidades que incluyen costo desconocido.
6. **Arrastre histórico:** detectar todas las entradas/salidas posteriores afectadas
   en orden históricamente acreditado del ledger, no solo la última receipt.
   Java no guarda una secuencia total de aplicación: `now()` es inicio de
   transacción y UUID aleatorio no es orden causal. Fecha+UUID ordena el export,
   pero no demuestra el orden de costeo. Proponer reconstrucción por balances
   `resulting_quantity` y vínculos exactos solo cuando determine una cadena única;
   ante ambigüedad, conservar historia y bloquear valoración confirmada hasta
   reconciliación/base económica autorizada, nunca escoger orden por conveniencia.
   Una compra conocida
   posterior no cura automáticamente el saldo ni el costo de ventas históricas.
   Reportes distinguen cantidad conocida, valor parcial y valor total desconocido;
   no publican margen confirmado para la cadena afectada.
7. **Regularización posterior:** evidencia autorizada permite un hecho económico
   nuevo, con razón, actor, fecha, política, comando y vínculo al movimiento
   original. No cambia receipt, snapshot ni delta físico; no duplica stock ni
   afirma que un costo tardío se conocía en la fecha original. Reconciliar cadena
   de consumo/residuales mediante asientos correlacionados, no reescritura.
8. **Operación sobre saldo sin valorar:** PRE0D-1 recomienda permitir continuidad
   física autorizada sobre legado íntegro pending, con seguimiento atómico y sin
   costo/margen confirmado. STRICT sigue rechazando insuficiencia; negativos y
   estimaciones requieren guards/evidencia explícitos. RESELLABLE afectado por
   déficit/pending se deniega sin efectos hasta un contrato específico. Policy
   puede estar ausente solo en seguimiento físico pending sin número económico.
   Esto requiere aceptación CC-0021: no habilita hoy checkout ni cambia el freeze.

### 3.1 Contrato de campos propuesto para discusión

Todos los nombres añadidos son **P**, no DDL autorizado. El mapping de 47 campos,
con marcas C/C-T/C-!/P, está en el contrato de compras; esta tabla cubre solo la
resolución económica. `TEXT exacto` significa decimal canónico validado en Rust,
no cast SQLite ni redondeo implícito PostgreSQL. Precisión/rango requieren contrato.

| Campo / ubicación candidata                  | Tipo lógico / físico Local                                                                  | NULL                           | Invariante propuesta                                                                                               |
| -------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| PO line `unit_cost_exact` existente en F1–10 | UnitCostExact / TEXT exacto                                                                 | SÍ, cambio de NN               | Costo pactado >=0 si existe; NULL desconocido, cero conocido.                                                      |
| PO line `line_total_exact` existente         | DecimalExact / TEXT exacto                                                                  | SÍ, cambio de NN               | NULL si costo desconocido; si conocido, producto exacto cantidad*costo, sin presentar como total facturado.        |
| Receipt line `unit_cost_exact` existente     | UnitCostExact / TEXT exacto                                                                 | SÍ para historia sin evidencia | Costo efectivo documentado, no precio pactado inferido.                                                            |
| Receipt line `cost_evidence_state` P         | enum / TEXT                                                                                 | NO                             | UNRECORDED_LEGACY o DOCUMENTED; estado inicial histórico inmutable. No default de evidencia conocida.              |
| Receipt line `valuation_state` P             | enum / TEXT                                                                                 | NO                             | PENDING_EVIDENCE, PROVISIONAL, CONFIRMED; estado original inmutable. Resolución posterior por hechos, no UPDATE.   |
| Procedencia de import P, asociada al hecho   | versión + identidad fuente + digest / representación por aprobar                            | NO para import                 | Identifica bundle, tabla, ID y limitaciones; no hash de contraseña ni token. No JSON en sustitución de relaciones. |
| Regularización económica P                   | vínculo tipado, importe exacto, moneda, política, actor, instante, razón, command ID/digest | Según ficha por aprobar        | Hecho append-only; FK/scope al movimiento; no delta físico nuevo. No se autoriza crear tabla/asiento incompleto.   |

No añadir un booleano genérico `migrated=true` para omitir invariantes. Cada
excepción se identifica por procedencia verificable y contrato de import versionado.
No representar un costo conocido únicamente mediante bandera sin importe/evidencia.

### 3.2 Estados y transiciones propuestos

| Flujo                            | Transición / regla                                                      | Efecto y prohibición                                                                                            |
| -------------------------------- | ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Import legacy sin costo efectivo | UNRECORDED_LEGACY + PENDING_EVIDENCE                                    | Preservar receipt/movement/stock una vez; sin asiento económico ficticio.                                       |
| Import legacy con costo pactado  | Preservar pactado; efectivo sigue no documentado si falta fuente propia | No ascender pactado a DOCUMENTED sin decisión/evidencia aprobadas.                                              |
| Nueva recepción                  | DOCUMENTED + CONFIRMED solo con política y cadena valorable             | Stock, valor, estado PO, audit, dedup/outbox en una transacción.                                                |
| Provisionalidad autorizada       | DOCUMENTED + PROVISIONAL, con base y razón explícitas                   | No equivale a falta total de dato; margen no definitivo.                                                        |
| Evidencia tardía válida          | Añadir regularización: pendiente/provisional → resuelto en proyección   | El estado del hecho original no cambia. Proyección reproducible con secuencia y política; guard CAS/command ID. |
| Evidencia insuficiente/duplicada | Rechazo / no-op del mismo comando y digest                              | Sin stock, valor, auditoría duplicados; mismo ID con contenido distinto falla.                                  |
| Corrección de una regularización | Nuevo asiento compensatorio autorizado                                  | Nunca UPDATE/DELETE del original; no backdating de conocimiento.                                                |

La PO conserva DRAFT→ORDERED→PARTIALLY_RECEIVED→RECEIVED y las cancelaciones
legadas válidas. CLOSED_PARTIAL y requisitos nuevos de motivo/valor no se
sintetizan retrospectivamente. Las recepciones Java son hechos publicados sin
columna de estado; su traducción propuesta a POSTED no significa valor CONFIRMED.

## 4. Consecuencias, riesgos y cambios del freeze

- Se preserva toda historia sin afirmar un valor que Java no guardó. Se amplía la
  representación de desconocimiento; no se altera Money exacto, UUID, ownership,
  single-writer, append-only ni el método promedio móvil congelado.
- Cambiar nulabilidad, representar pending y distinguir política de recepción
  nueva **sí requiere aprobación**, físico reconciliado y actualización de
  F1–10/F1–12/F1–11 y referencias de F4–21. Ver
  [CC-0021](../../05-development/rust-adr0021-change-control.md).
- El diccionario y sus 21 preguntas tienen respuestas técnicas recomendadas en
  el anexo D02; siguen pendientes de aprobación, no son cinco migraciones listas.
  La relación fiscal y seguimiento económico adicionales se incluyen expresamente
  en el cambio solicitado, no se agregan por inferencia.
- No afirmar que aceptar cinco padres produce necesariamente el modelo definitivo:
  aritméticamente daría 87 fichas (63/9/15) sin otros cambios, pero puede requerir
  relaciones/representaciones adicionales. No sustituir 83 por otro número meta.
- El import debe validar toda su unidad transaccional antes de commit, conservar
  identidades/digests y reimportar sin duplicar. Ni un export fiel ni la aceptación
  de este ADR acreditan import/paridad.
- No cambia HTTP ahora. Cualquier nuevo estado/campo visible exige contrato
  versionado, OpenAPI 3.1, ApiError y pruebas diferenciales antes de implementación.
- No habilita datos productivos, endpoints de negocio, sync Pro, Stripe o Control
  Plane comercial. Free nunca dependerá de red/licencia para corregir este diseño.

## 5. Pruebas obligatorias tras aprobación

1. PO con NULL/cero/positivo en DRAFT, ORDERED, PARTIALLY_RECEIVED, RECEIVED y
   CANCELLED; conservar snapshots, total desconocido y escalas (producto puede
   requerir 10 decimales, no solo 4).
2. Import de receipt sin costo propio: cantidad/ID/estado correctos, sin asiento
   cero, sin costo de catálogo; rechazar representación no aprobada sin efectos.
3. Evidencia tardía: regularización idempotente, autorizada, cross-org/branch
   rechazada, no muta hechos, no duplica cantidades, corrige proyección sin
   convertir márgenes históricos desconocidos en confirmados retroactivamente.
4. MWAC por item/branch, conocido cero, pendientes intercalados con salidas,
   Q=0, Q<0 autorizado, residuo/overflow/moneda y precisión; ningún float.
   Timestamps coincidentes/invertidos y concurrencia: distinguir orden de export
   de orden causal; reconstrucción ambigua no produce valoración confirmada.
5. Renombrar proveedor/item/unidad/regla actual no cambia snapshots. Resolver
   movimiento por relación exacta, no por fecha. Importar ledger no ejecuta receipts.
6. Fallo en cualquier efecto revierte stock/valor/estado/audit/dedup/outbox;
   reintento mismo digest no-op, digest diferente conflicto, concurrencia/CAS.
7. Constraints/FK/índices/append-only por ambos motores y aislamiento del Control
   Plane; benchmarks de auth independientes, sin secretos en reportes.
8. DDL13-01..11, PAR-01..09 y AS-01..10 reales sobre esquemas completos,
   offline con red denegada, upgrade y restore exactos. `--require-complete`
   continúa devolviendo 2 hasta completar todos los obligatorios.

## 6. Rollback y aprobación

Hoy no hay DDL ni datos de este ADR que revertir. Se puede descartar únicamente
la propuesta local mediante un cambio revisado, conservando el historial. Java
sigue autoridad; no hay doble escritura. No revertir por iniciativa propia los
ocho commits publicados del baseline reconciliado ni modificar una migration aplicada.

Tras aprobación: baseline documental versionado y huellas, snapshot sintético
N-1, backup consistente, migraciones aditivas/inmutables y restore ensayado antes
de cualquier uso real. Si falla import, rollback completo y verificar digests;
si falla upgrade, no abrir operaciones, restaurar copia verificada o forward-fix
revisado. No borrar hechos económicos nuevos mediante down migrations destructivas.

**Aceptación requerida:** titular registra opción, alcance histórico/nuevo,
contratos numéricos/operativos y decisiones Q del diccionario; responsables de
datos/dominio/seguridad revisan; CC-0021 referencia versiones canónicas aprobadas.
Hasta entonces permanece PROPOSED, import afectado bloqueado y ningún gate cambia.

## 7. Fuentes normativas y trazabilidad

Consultadas por export TXT en 0c (URLs mutables); huellas principales en el
[diccionario](../../05-development/rust-missing-physical-proposal.md#22-huellas-de-evidencia).
No se copian expedientes completos ni se afirma acceder al historial de Drive.

| Fuente                                                                                        | Secciones / uso                  | Autoridad y límite                                                                   |
| --------------------------------------------------------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------------ |
| [F4–21](https://docs.google.com/document/d/1tIHbbRw5GhKOFkL4ff0Zoc-SmMc-q7J2WwyYXFjqiUE/edit) | §1–7, freeze, IMP-01..09         | Aprobado; cambios de invariantes exigen control.                                     |
| [F1–10](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/edit) | §16.16–19,37–41,60; §17 y anexos | Físico congelado con brechas; no deducir campos.                                     |
| [F1–11](https://docs.google.com/document/d/1PCgbE95Qt3GjIfy78rPt-tCMjQ2Ed2GwBzEcTfa4HMU/edit) | §4–7 y ratificación              | Constraints/gates; pruebas pendientes no reabren freeze salvo discrepancia material. |
| [F1–12](https://docs.google.com/document/d/16hyT4XALY3DrAc50RUTx-fESzMI0c5612zTKphHRieM/edit) | §2,4                             | 37/37, costo nullable, snapshots, exactitud.                                         |
| [F1–13](https://docs.google.com/document/d/1ZtDnxOeVjeBJD20gLI1GxA4cTY-bevRoL2fKUMqZBPk/edit) | §4–8                             | 30 escenarios y artefactos, no aprobación por checks técnicos.                       |
| [F4–13](https://docs.google.com/document/d/1vL-yFrwenxeqbAjuWY9m5zOMhlMGADfVTDUeWaOa4OM/edit) | §7–10,13 y ratificación          | Promedio móvil, cantidad/valor y compensación; no regla NULL→0.                      |
| [F4–15](https://docs.google.com/document/d/12VYpuy-IznZufJpYiGqn-4BBi3JqeI0al3XCE5Gx7tU/edit) | §4–6                             | Transiciones y scopes, atomicidad de efectos futuros.                                |
| [F4–18](https://docs.google.com/document/d/1IqeQSJlpzaf7qEsA1tcFIL1hVMa1H8lzc6IAarHKzlw/edit) | §1,3–6 y ratificación            | SQLx independiente; Flyway legado inmutable.                                         |
| [F4–19](https://docs.google.com/document/d/1TTiJo3tHgbEdB043mLQKTayi8zEmP_WIk0vdshP2Bgk/edit) | §2–7 y ratificación              | BCrypt→Argon2id, lockout, no tokens Java; hardware/policy aún por validar.           |
| ADR-0020, Java V1–V25 y tests citados                                                         | AS-IS y frontera Rust            | No sustituyen el físico normativo; ninguna aceptación nueva inferida.                |
