# Contrato propuesto de aprobación de valoración — D03–D06 / 0d

**PROPUESTO (P), NO APROBADO. Revisión de integración: 2026-10-28.** Rama
`feat/backend/rust-0d-readiness`, base/HEAD inspeccionado `329c098`.
Único write set propio: este archivo. Sin código normativo, DDL, cambios de API,
import real, producción, aceptación del ADR ni modificación de gates.
Java sigue siendo autoridad; hoy continúa el bloqueo conservador de import afectado.
**Autoridad propuesta D03–D06:** este anexo selecciona `MWAC-DEC1`; sustituye su
recomendación anterior MWAC-R1, no el freeze vigente. [D02](rust-d02-approval-contract.md)
§5 refiere este contrato sin duplicar cálculo; `calculation_contract_version=MWAC-DEC1`
es el nombre común. D02 conserva ownership/lifecycle de CostingPolicy; no se edita aquí.

## 1. Autoridad, fuentes vigentes y límites

Leídos [AGENTS](../../AGENTS.md), [ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md)
§1–6, [CC-0021](rust-adr0021-change-control.md) §2–4 y
[contrato de compras](rust-purchase-parity-contract.md) §1–9 (47/47 columnas).
**N** identifica baseline ratificado; **H**, hecho AS-IS; **P**, elección de este
anexo sometida a aprobación. Toda regla nueva de §2–9 es P, no una norma vigente.

| Fuente consultada por export TXT en esta sesión                                                            | Secciones utilizadas / autoridad N                                                                                                   |
| ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| [F4-13](https://docs.google.com/document/d/1vL-yFrwenxeqbAjuWY9m5zOMhlMGADfVTDUeWaOa4OM/export?format=txt) | §3–4, §7–10, §13 y ratificaciones: MoneyMinor, operandos exactos, MWAC Branch/Item, STRICT, negativos provisionales, compensaciones. |
| [F4-15](https://docs.google.com/document/d/12VYpuy-IznZufJpYiGqn-4BBi3JqeI0al3XCE5Gx7tU/export?format=txt) | §2–6 y ratificaciones: terminales, receipt POSTED, atomicidad, scopes/FK compuestas; RBAC no lo sustituye una FK.                    |
| [F1-10](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/export?format=txt) | §2–3, §6–7, §10, §16.14–18/21–23/40–41: físico congelado, costos NN incompatibles y ledger económico decimal.                        |
| [F1-12](https://docs.google.com/document/d/16hyT4XALY3DrAc50RUTx-fESzMI0c5612zTKphHRieM/export?format=txt) | §2, §4, §6 y freeze: nullable pactado, UUID/snapshots/cantidades exactos, PAR-01..09.                                                |
| [F4-21](https://docs.google.com/document/d/1tIHbbRw5GhKOFkL4ff0Zoc-SmMc-q7J2WwyYXFjqiUE/export?format=txt) | §1–2, §5–7: Free offline, SQLite autoridad, single writer, cambio por ADR, gates sin dispensas.                                      |

Bytes/SHA-256 de la consulta fuente conservada; sin copias del expediente en el repo:

| Fuente | Bytes | SHA-256                                                            |
| ------ | ----: | ------------------------------------------------------------------ |
| F4-13  | 29625 | `2574340697c13d20b98f1188e706cae2ccf2147c37a737806d58d7788be96cc2` |
| F4-15  | 16597 | `0eac0adb025c6865c87612896d92cbcb1f43318fc141a871c28b439415d8e6f4` |
| F1-10  | 94378 | `8a3354af4c1634ad0c97ebccb8be787202e67423d37cc5df548d4137e4549e5f` |
| F1-12  | 17018 | `e0dfa203c4c4ca5bc66b3b4ea3a9e7cced017ef300a9367295836128a5af5729` |
| F4-21  |  9597 | `b1912c884ec384b62944ca366810eaf809fcd257e6bde85744d5c15e7978548d` |

URLs mutables: estas huellas identifican lectura, no revisión privada ni aprobación.
Ratificaciones finales prevalecen sobre «PROPOSED/todos abiertos» históricos; no
fijan escala 18, HALF_EVEN de costeo, umbral ni followup aquí elegidos.
H contrastado: `PurchaseOrderAmounts` admite NULL/cero/NUMERIC(19,4);
`InventorySaleConsumptionService.consume` aplica STRICT antes de mutar. El comentario
antiguo de `InventoryNegativeStockPolicy` no prueba que checkout ignore la policy.

## 2. Decisiones propuestas, alternativas y razón

| Decisión | Propuesta concreta seleccionada P                                                                                                       | Alternativa / razón para no recomendarla                                                                                                                                             |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D03      | **B+D**: pactado opcional y ausencia efectiva histórica tipada; conservar hechos sin inventar valoración.                               | A: bloqueo seguro hoy, sin resolver paridad. C: exigir costo retrospectivo excluye Java válido. E: cero/catálogo/promedio como efectivo falsifica historia.                          |
| D04      | PO nueva emitible sin pactado. Receipt nueva exige evidencia local propia; sin ella, denegar POSTED, permitir trabajo no contabilizado. | Recepción nueva solo física amplía desconocimiento; exigir factura/remoto impide Free. Se admite atestación local explícita, no fallback.                                            |
| D05      | Continuidad física autorizada sobre legado reconciliado sin valor, con pending atómico; ningún costo/margen confirmado ficticio.        | Bloquear toda venta por falta histórica de costo paraliza Free; estimación automática confunde pending con provisional.                                                              |
| D06      | **MWAC-DEC1**: V/deltas escala 18, cociente exacto temporal y HALF_EVEN explícito; agotamiento toma todo V.                             | Racional persistido MWAC-R1 de hasta 512 dígitos: preserva MWAC matemático, pero amplía físico/tipos/CPU y complejidad; límite de denominador puede detener operaciones. No elegido. |

DEC1 mantiene ledger decimal y usa **un único `valuation_followup` append-only** para
pendiente/evidencia/cálculo/resolución, junto a ValuationEntry; no cuatro tablas nuevas
por clase de evidencia. A cambio, **no promete equivalencia con MWAC racional sin
cuantización**: exactitud de fuentes, saldos registrados y conservación ≠ costo ideal
matemático sin redondear. Confirmado significa conforme a evidencia y policy DEC1.
La escala/cota, followup y excepción pending requieren CC21; no son aceptación tácita.

## 3. D03–D04: representación mínima y evidencia offline

Pactado = intención; efectivo = costo de entrada documentado; pending = falta de
base/cadena; provisional = estimación numérica explícita no definitiva. Efectivo
conocido no acredita todo V. PO conserva juntos costo/total NULL; si c existe,
total=q×c exacto, no factura/pago. No completar desde catálogo ni promedio.

Campos P, no fichas canónicas/DDL. UUID TEXT/UUID; instante UTC TEXT/TIMESTAMPTZ;
enum TEXT. Todo registro branch-owned tiene org/branch/item y claves/FK scoped
según F4-15/D02; moneda/unidad históricas, sin default de evidencia/CONFIRMED.

| Ubicación / campos concretos P                                                                                                                                                          | Tipo, NULL y vínculo / condición                                                                                                                                                                                                                             |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| PO line `unit_cost_exact, line_total_exact`                                                                                                                                             | Decimal(19,4)/(38,10), ambos nullable; moneda PO y unidad snapshot.                                                                                                                                                                                          |
| Receipt line `unit_cost_exact, cost_evidence_state`                                                                                                                                     | c nullable solo historia sin evidencia propia; estado NN UNRECORDED_LEGACY/DOCUMENTED. Nueva receipt: c NN y evidencia local vinculada obligatoria; sin policy aplicable, soporte documental en PENDING sin importes, no EVIDENCE valorada.                  |
| Receipt/consumo `initial_valuation_state, assessment_origin, assessed_at, pending_reason`                                                                                               | NN estado/origen/instante; PENDING_EVIDENCE/PENDING_CHAIN/PROVISIONAL/CONFIRMED; AT_POST/AT_IMPORT. Razón NN salvo confirmado; import clasifica HOY, no inventa estado Java.                                                                                 |
| `valuation_followup`: `id, organization_id, branch_id, item_id, movement_id, entry_kind, root_followup_id, revision, operation_ordinal`                                                 | UUID PK, FK al movimiento NN; kind NN PENDING/EVIDENCE/CALCULATION/RESOLUTION/DEFICIT/RESIDUE_ADJUSTMENT. Root es self-FK (primera fila a sí misma); revisión≥1; UK(root,revision,ordinal) scoped.                                                           |
| Followup `supersedes_followup_id, evidence_followup_id, basis_followup_id, receipt_line_id, cause_movement_id`                                                                          | FK tipadas nullable solo cuando no aplican; self-FK scoped verifican kind/linaje. Receipt EVIDENCE exige receipt_line_id; basis/cause refieren hecho exacto, no TypedRef sin FK.                                                                             |
| Followup `costing_policy_id, calculation_contract_version, recorded_at, actor_id, reason, command_id, payload_digest, scope_sequence`                                                   | Policy/contrato NN salvo ambos NULL solo en PENDING físico sin valoración, razón NO_APPLICABLE_POLICY. Actor FK/fecha/cmd/digest/secuencia NN en toda operación nueva, también esa excepción; manifest conserva ausencia fuente sin inventar ejecución Java. |
| Followup EVIDENCE `evidence_kind, source_reference, artifact_ref, artifact_digest, document_issued_at`                                                                                  | SUPPLIER_DOCUMENT/LOCAL_ATTESTATION/CAUSAL_ORDER, referencia/artefacto local/digest NN; fecha documental nullable si no consta. Captura/actor son los actuales, no backdate.                                                                                 |
| Followup import `bundle_id, bundle_digest, source_table, source_id, source_row_digest, mapping_version, source_scale`                                                                   | NN para legado; preserva representación/NULL originales y enlace al movimiento/receipt, sin UUID/actor/fecha fuente sintéticos.                                                                                                                              |
| Followup datos de costo/ajuste `quantity_exact, unit_cost_exact, total_exact, adjustment_amount_exact, currency, base_unit, counterpart_code`                                           | q/c/total NN para evidencia de entrada; total=q×c. Ajuste usa importe(76,18) firmado y contrapartida, no recepción ficticia. Currency/unidad NN donde aplica.                                                                                                |
| Followup CALCULATION `input_value_exact, input_quantity_exact, allocated_quantity_exact, rounded_amount_exact, rounding_remainder_integer, rounding_direction, error_bound_half_quanta` | Inputs/resultados decimales §5; resto entero TEXT/NUMERIC(19,0), dirección −1/0/+1; presupuesto i64 NN. Resultado decimal18 cuantizado declarado, no fracción persistida n/d; input Q reconstruye denominador.                                               |
| Followup DEFICIT `remaining_quantity_exact, remaining_estimate_exact, estimate_basis_movement_id`                                                                                       | Cantidad NN; estimación(76,18) y base FK nullable solo si sin base. Liquidación añade fila sucesora; nunca modifica deuda anterior. Presupuesto de error acompaña estimación.                                                                                |
| ValuationEntry/CostSnapshot `amount_exact, calculation_contract_version, calculation_followup_id, error_bound_half_quanta`                                                              | Importe(76,18) NN; FK scoped a CALCULATION/RESOLUTION NN en DEC1 nuevo. Movimiento/policy FK obligatorias; snapshot inmutable, sin costo numérico para pending.                                                                                              |
| Proyección `quantity_exact, value_exact, average_display_exact, state, as_of_sequence, knowledge_revision, error_bound_half_quanta`                                                     | Q NN; V NULL si desconocido; promedio NULL si Q≤0/pending, de presentación a 12. Presupuesto NULL cuando valor desconocido, no 0 que afirme exactitud.                                                                                                       |

Los escalares variables de evidencia/trace pueden ir en payload versionado del mismo
followup (máximo P 64 KiB), no en tablas distintas; FK, kind, revisión, scope y claves
consultadas siguen columnas tipadas. Artefactos duraderos locales por referencia/digest,
no URL remota obligatoria. Revisión/estado actual se proyecta de filas nuevas, no UPDATE.
Una corrección referencia evidencia previa y supersedes; ningún borrado de evidencia.

Nueva receipt exige artefacto accesible offline antes de commit (staging duradero
verificado), proveedor/cantidad/unidad/moneda/base y actor autorizado. Atestación local
autenticada/auditada sirve P, no certificación fiscal; gratuidad y adopción del pactado
se atestan expresamente para esa recepción. Sin flete/impuesto recuperable capitalizado
implícito. Import: manifest fiel acredita ausencia, no costo efectivo; PO conocida no
vuelve DOCUMENTED una receipt sin dato propio. Aporte tardío = EVIDENCE+RESOLUTION
nuevos, no UPDATE. POSTED importado acredita hecho físico, no valor confirmado.
Digest nuevo incluye inputs/evidencia/policy; fingerprint Java se conserva aparte.

## 4. D05: matriz operativa Free propuesta, sin red

Permitir exige sesión/usuario ACTIVE, membership/RBAC, turno/estado/version/unidades/
moneda válidos y stock policy local. Permisos P separados `inventory.valuation.resolve`
y `inventory.negative.consume` (este además de ALLOW_NEGATIVE), no derivados del plan.
Sin policy aplicable (retirada sin sucesora incluida): solo kind=PENDING físico,
estado=PENDING_CHAIN, NO_APPLICABLE_POLICY, ambos NULL; sin importes/estimación/trace valorado/ValuationEntry.
Actor/cmd/seq/audit obligatorios. Todo importe económico o RESOLUTION exige política aprobada/seleccionada
aplicable y contrato NN; ni par parcial NULL ni fallback a retirada. D04 conserva c/evidencia de entrada, sin autorizar valoración.

| Caso                                                   | Permitir / denegar P y efecto atómico local                                                                                                                               |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Continuidad física sin policy aplicable                | Permitir solo PENDING sin valoración con ambos NULL y NO_APPLICABLE_POLICY, actor/cmd/seq/audit; no cálculo ni RESOLUTION. D04/RBAC/stock y demás guards siguen vigentes. |
| Emitir PO sin pactado / cero / positivo                | Permitir; sin stock/caja/valor; total NULL/cero/producto exacto.                                                                                                          |
| Receipt nueva sin evidencia admisible                  | Denegar POSTED completo; permitir guardar trabajo no contabilizado.                                                                                                       |
| Receipt documentada, cadena completa Q≥0               | Permitir stock+valor DEC1+PO+followup trace+audit/event/dedup/outbox.                                                                                                     |
| Receipt documentada sobre legado pending/ambiguo       | Permitir físico/evidencia y PENDING_CHAIN/followup atómico; sin ledger numérico hasta resolver cadena.                                                                    |
| Venta/receta/merma con Q suficiente, V desconocido     | **Permitir continuidad física pending**, seguimiento por consumo/componente, sin costo/margen confirmado ni provisional cero; merma no paga caja.                         |
| Salida insuficiente bajo STRICT                        | Denegar toda operación; desconocer valor no relaja stock. Import negativo preserva hecho, no autoriza consumo nuevo.                                                      |
| Venta ALLOW_NEGATIVE con permiso adicional             | Permitir déficit PROVISIONAL solo con policy aplicable y base §6; sin base, PENDING_EVIDENCE; sin policy, PENDING_CHAIN/NO_APPLICABLE_POLICY, ningún importe ficticio.    |
| Merma/conteo/manual crearía negativo                   | Denegar aun con ALLOW_NEGATIVE; no usar excepción de venta como bypass de otro proceso.                                                                                   |
| Receipt documentada reduce negativo                    | Permitir aunque Q siga <0; liquidación causal o pending si cadena no acreditada; no añadir guarda de proveedor activo por analogía con emisión.                           |
| Evidencia tardía/corrección económica                  | Permitir permiso+evidencia+CAS+orden acreditado; ambiguo deniega confirmación, no continuidad física válida.                                                              |
| RESELLABLE con asignación/pool no afectados            | Permitir solo snapshot valorado sin vínculo a déficit/pending y baseline vigente sin pending/Q<0/deuda abierta, policy aplicable y guards de postventa.                   |
| RESELLABLE afectado o no acreditable                   | Denegar completo antes de efectos, incluso si déficit original fue regularizado; no aplicar Q/V sin reversión de deuda, diferida fuera de alcance.                        |
| QUARANTINE / DAMAGED                                   | Registro físico separado solo si disposición/flujo aprobado lo admite; no toca Q/V del pool ni deuda. No fallback automático ni nueva capacidad de almacén.               |
| Lectura/reimpresión/caja/export con costo pending      | Permitir según RBAC; pagos exactos independientes; reportes etiquetan ausencia/cobertura y no afirman utilidad confirmada.                                                |
| Scope/moneda/unidad inválido, overflow o cota excedida | Denegar comando valorado entero; no degradar a pending para ocultar error. Leer/exportar sigue permitido.                                                                 |
| Mismo command/digest / mismo command distinto digest   | Resultado original sin duplicados / conflicto, comprobando autorización vigente.                                                                                          |

Pending no tiene estimación válida. Q puede conocerse, V/MWAC/margen total no;
subtotal conocido etiquetado no equivale a total. Conservar estado capturado y mostrar
«conocido entonces» frente a «resuelto revisión R». Red/licencia/Stripe/Control Plane
no deciden estos casos. Stock, followup, audit/event/dedup/outbox son atómicos incluso
cuando no puede existir ValuationEntry; esa excepción es P, no regla ya aprobada.

## 5. D06: contrato numérico propuesto `MWAC-DEC1`

| Magnitud                          | Precisión, escala y rango P                                                                                                                                                     |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q, cantidades                     | Decimal(19,6), máximo absoluto 9999999999999.999999; q/s>0; signo en saldo/delta. Nuevos UNIT enteros; preservar 6 decimales históricos sin reinterpretar unidad.               |
| c pactado/efectivo                | Decimal(19,4), 0..999999999999999.9999; NULL≠0. c negativo rechazado; rebaja posterior es nueva evidencia/ajuste.                                                               |
| POtotal y q×c                     | Decimal(38,10), máximo (10^19−1)²/10^10; entrada a V extiende ceros hasta 18, sin redondear producto.                                                                           |
| V, C, estimación y deltas/ajustes | Decimal(76,18) firmado: −M..M, M=10^58−10^−18. Persistencia y suma/resta exactas en esta rejilla; cota de error no cambia su escala.                                            |
| Cociente/intermedios              | Enteros/racional exactos SOLO temporales; hasta 128 dígitos enteros por intermedio, checked. Vint×sint necesita hasta 95; ninguna división decimal truncada antes de cuantizar. |
| MWAC visible                      | HALF_EVEN(V/Q,12), marcado ≈; string solo presentación, nunca operando ni sustituto de base V/Q.                                                                                |
| MoneyMinor independiente          | i64 firmado completo; currency/minorDigits de catálogo versionado (0..4). Histórico solo convertible si x×10^minorDigits es entero en rango.                                    |

Strings canónicos sin exponente/espacios/NaN/Infinity/float/JSON Number, sin +/ceros
iniciales/−0; escala fija 6/4/10/18 según tipo. Escala fuente se conserva en import.
Validar forma/rango antes de persistir: PG no puede redondear silenciosamente.
Overflow de input, temporales, acumulados, ajuste, carry al cuantizar o presupuesto
causa rollback total. Sin saturación, recorte, conversión a 0 ni fallback de policy.

Sea h=10^−18. Con Q>0/cadena completa, receipt: Q'=Q+q, V'=V+q×c exactos.
Salida 0<s<Q: **C=quantize_HALF_EVEN(V×s/Q,18)**; Q'=Q−s, V'=V−C exactos.
**Último consumo s=Q: C=V, Q'=V'=0**, sin recalcular ni añadir compensación residual.
No multiplicar s por el MWAC mostrado. Snapshot C y delta −C se congelan con policy.
UnitCostExact(19,4) describe c de entrada, no el promedio derivado: snapshot de consumo
conserva V/Q como inputs decimales, sin inventar otro operando unitario redondeado.
Conservación en valores registrados: Vfinal=Vinicial+Σentradas−ΣC+Σajustes exactamente.
Q=0 => promedio NULL; V conocido≠0 => VALUE_RESIDUE, no confirmado: solo ajuste
explícito autorizado −V/contrapartida VALUATION_DIFFERENCE; nunca ocultarlo en venta.
Pending no se cura por Q=0/compra conocida; requiere resolver su cadena/históricos.

Trace obligatorio por asignación: para Vint=V/h, sint=s×10^6, D=Q×10^6,
N=Vint×sint; t=trunc_hacia_0(N/D), r=N−tD. Si 2|r|>D incrementar t según signo;
si iguales, hacerlo solo si t es impar. Guardar inputs, r, dirección y resultado;
δ=(C/h−N/D)×h es reconstruible, no fracción económica persistida. EXHAUST registra
C=V/resto 0. Receipt registra producto exacto; no basta un «redondeado=true» genérico.
HALF_EVEN firmado vale también en ajustes/retornos; r=0 implica δ=0.

### Cota y umbral P: no equivalencia matemática ilimitada

En salida parcial |δ|≤h/2. Frente a MWAC ideal con las mismas entradas exactas,
E'= (1−s/Q)E−δ; tras k salidas parciales, |E_V|≤k×h/2 (receipt exacta no añade error).
Con entradas exactas, diferencia del C total acumulado = −E_V por conservación;
al agotarse pool, E_V=0. Cada C individual puede diferir del ideal; agotamiento no
hace exactos retroactivamente los snapshots parciales ni justifica compensarlos.

Persistir cota conservadora **B entero en medias quanta**, error≤B×h/2, no racional.
Fuente exacta B=0; escala exacta por a≥0: B'=ceil(aB); suma/diferencia: sumar cotas.
En split positivo: B_C=ceil((s/Q)B_V)+b, B_V'=ceil((1−s/Q)B_V)+b, donde b=1 si r≠0,
0 si r=0. EXHAUST: B_C=B_V, B_V'=0. Entradas con incertidumbre heredada (retorno,
estimación) SUMAN su cota; no resetear B al recibir stock. Esta regla también cubre
asignación de deuda y evita aplicar la cota simple k a ramas con error heredado.
Umbral explícito: **B≤20000000000**, error acotado ≤0.00000001 unidades monetarias,
para V/C/deuda/ajuste publicado; excederlo deniega antes de commit. B en i64 checked;
ceil se calcula con enteros exactos. Resolución guarda cotas objetivo recalculadas,
y ajuste hereda suma de cotas vieja+nueva, sin afirmar independencia/cancelación.
Cota = diagnóstico conservador, no evidencia de costo ni tolerancia para diferencias
sin explicar; su acumulación puede bloquear comandos y exige revisión, no auto-reset.

**Exactitud histórica de fuentes:** no redondear/truncar/reemplazar UUID/snapshots,
NULL/decimales/tiempos del mapping 47/47. Operand(19,6)/(19,4) válido cabe; q×c cabe
(38,10). Money histórico MXN 1.0001 o fuera de i64 conserva original y bloquea import
operativo de su unidad, nunca se fuerza a centavos. Valorar posteriormente con DEC1
es cálculo nuevo cuantizado identificado, no afirmar que Java ejecutó DEC1 ni que
el costo ideal racional periódico era decimal finito. No recostear un hecho ya
registrado con otra policy para aparentar paridad. Revisión matemática no es gate.
Reporte monetario de costo: sumar decimales18 y HALF_EVEN a minorDigits; reparto por
mayores restos sobre pisos, empate scope_sequence/ordinal, sin asiento por redondeo
visual. Fiscal/precio/cobro conservan políticas propias, no heredan DEC1.

## 6. Negativos, pending y costo tardío bajo DEC1

No usar V/Q si Q<0. Al cruzar cero, tramo disponible toma todo V; déficit d abre
DEFICIT con estimación G=quantize(Vbasis×d/Qbasis,18), base = último balance positivo
completamente confirmado del scope, referenciado con Vbasis/Qbasis/trace/cota, no
promedio12 persistido como operando. Si nunca hubo base, no G: pending sin asiento.
No usar esa base para inventar efectivo ni para curar desconocimiento legado.

Incidente conserva deuda D y estimación restante G en filas sucesoras del followup.
Receipt q,c cubre deudas por orden causal de apertura (no FIFO de stock positivo).
Para x=min(q restante,D), asignar A=quantize(G×x/D,18), o A=G si x=D;
costo efectivo cubierto T=x×c exacto. Entrada q×c se registra una vez; corrección
ΔV=A−T y ΔCOGS=T−A, correlacionadas al consumo/receipt. D'=D−x, G'=G−A exactamente.
V negativo provisional=−ΣG; deuda agotada no deja residuo de estimación. Solo tras
saldar toda deuda, sobrante de receipt forma pool positivo a c. Sin G, diferir ledger
hasta resolución y materializar T como primer costo, no compensación contra cero
conocido. Propagar cotas §5 en G/A/ajustes; final con costo documentado no hereda una
estimación descartada, pero sí incertidumbre del tramo positivo consumido original.

Pending positivo difiere asientos dependientes, no divide V viejo por Q nuevo.
Evidencia tardía permite replay **DEC1**, no MWAC racional alternativo, desde baseline
acreditado y misma secuencia, incluyendo todas las salidas/entradas/mermas/deudas/
retornos/resoluciones posteriores. RESELLABLE solo en el caso no afectado de §4:
asignar costo elegible remanente del snapshot con quant18/último tramo exacto y cota.
Si baseline/pending/deuda afecta la asignación, o no puede excluirse, denegar: reversión
de deuda referenciada queda diferida, aun con costo luego resuelto o Q actual≥0.
Rechazo atómico: sin ReturnLine/cantidad elegible consumida, Q/V/deuda/snapshots,
ledger/dedup/secuencia committed nuevos; jamás completar parte del intento ni cambiar
a QUARANTINE automáticamente. Solicitud física QUARANTINE separada según flujo ya
aprobado no toca pool/deuda. Esto no paga/cancela/crea caja ni refund: su obligación
y elegibilidad se rigen por postventa aparte, no se autorizan ni deniegan por este guard.
Resolución calcula objetivo por movimiento y diferencia contra suma registrada:
añadir solo deltas(76,18), sin redondear de nuevo diferencias ya representables;
importe antes ausente recibe primer asiento. Agrega RESOLUTION/trace/ValuationEntry,
no tabla adicional ni UPDATE del snapshot/estado inicial. Conservar «entonces» y
«revisado», fecha de conocimiento actual; jamás Q/pagos duplicados ni backdate.
Replay al watermark de scope: CAS de secuencia/revisión antes del commit conjunto;
si cambia, recalcular candidato, no aplicar deltas obsoletos. Umbral/rangos aplican
también al replay; ningún ajuste puramente de redondeo para forzar MWAC ideal.

## 7. Secuencia causal nueva frente a historia H

P: `scope_sequence` i64 por (org,branch,item), `predecessor_sequence, command_id,
operation_ordinal, writer_generation, quantity_before/after, knowledge_revision`
y digest. Contador/proyección/efectos se escriben bajo writer owner y misma TX,
asignados al serializar, no por reloj/recepción HTTP. Correcciones solo económicas
también consumen secuencia. Overflow deniega; rollback no consume contador.
Bloquear múltiples scopes en orden estable y validar todas las versiones. SQLite
BEGIN IMMEDIATE+CAS; PG lock de fila del contador+CAS, **no nextval** como orden de
commit. Outbox origin_sequence mantiene contrato distinto; referenciar ambas.

H Java no guarda secuencia total. No inventarla por fecha/UUID/orden de archivo/
txid supuesto. Conservar delta/resulting_quantity/enlaces; arista de Qprev=result−delta
a result. Desde apertura/baseline verificado hasta saldo final, exigir cadena única
que use todos los movimientos una vez y respete precedencias acreditadas. Balances
repetidos/ciclos pueden admitir órdenes distintos aunque timestamps sean diferentes.
Buscar segunda cadena: si existe, AMBIGUOUS_ORDER; ninguna => inconsistencia física,
import operativo bloqueado. Presupuesto P 100000 estados por scope: agotarlo no
prueba unicidad. Sin una cadena completa e integridad verificada, no import operativo;
una cadena sin probar unicidad solo permite pending, nunca valor confirmado.
No elegir ruta por margen/greedy. Ambiguo íntegro conserva fuente y continuidad §4.
Manifest conserva grafo/evidencia vía followup CAUSAL_ORDER, sin tabla extra. Índice
único reconstruido es DERIVADO con digest/prueba, no secuencia original Java. Futuro
cutover: baseline manifest enlaza saldo importado con nueva secuencia 0/pending;
comandos nuevos empiezan 1 sin renumerar pasado. Evidencia de orden exige fuentes,
no mera selección de permutación. Apertura económica futura autorizada no confirma
COGS/snapshots antiguos ambiguos ni los hace exactos por compartir saldo final.

## 8. Vectores DEC1 propuestos (16), todos sintéticos

MXN/unidad base salvo indicación; R(q,c)=receipt, S(s)=salida; h=10^−18.
Decimales de V/C son valores registrados, fracciones solo oráculo temporal ideal.
Resultados propuestos, no pruebas de integración/gates ejecutados.

| ID  | Datos/orden acreditado                                                   | Resultado requerido P                                                                                                                                                                                                                                                                           |
| --- | ------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| V01 | PO q=2; c=NULL / 0 / 1.25                                                | Total NULL / 0 / 2.5000000000. Receipt importada sin evidencia propia sigue efectivo NULL/pending en los tres casos.                                                                                                                                                                            |
| V02 | R(0.000001,0.0001), S(0.000001)                                          | Entrada V=0.000000000100000000, Q=0.000001, MWAC visible 0.000100000000; EXHAUST C=V, Q=V=0, B=0.                                                                                                                                                                                               |
| V03 | q=(10^19−1)/10^6, c=(10^19−1)/10^4; Vmax=M                               | q×c=(10^38−2×10^19+1)/10^10 exacto; V extiende 8 ceros. Qmáx+0.000001, c negativo/escala5, M+h, carry quant18(M+h/2), i64máx+1 deniegan. B=20000000000 admite cota 10^−8; B+1 deniega. Histórico MXN 1.0001 no se fuerza a minor.                                                               |
| V04 | R(10,20), R(10,30), S(4), merma(2)                                       | Q,V: (20,500)→(16,400)→(14,350); MWAC 25.000000000000; C=100/50 exactos DEC1, B=0, CashOut merma=0.                                                                                                                                                                                             |
| V05 | R(1,1), R(2,0), S(1), R(1,1), S(1), S(2)                                 | C1=0.333333333333333333, V=0.666666666666666667; nueva entrada V=1.666666666666666667; C2=0.555555555555555556, V=1.111111111111111111; EXHAUST C3=V, final0, ΣC=2. Con D=3000000, restos primeros=1000000/2000000 y direcciones0/+1. No son 1/3,5/9,10/9 matemáticos exactos.                  |
| V06 | quant18(±h/2), quant18(±3h/2); R(1,1),R(2,0), tres S(1)                  | Empates: 0/0 y +2h/−2h. C secuenciales 0.333333333333333333 / 0.333333333333333334 / 0.333333333333333333, suma1; reparto minor 33/34/33 por mayor resto. Reporte HE de 1.005/1.015/−1.005 =>1.00/1.02/−1.00.                                                                                   |
| V07 | Baseline Q=0,V=0.010000000000000000                                      | MWAC NULL, VALUE_RESIDUE; ajuste autorizado −0.010000000000000000 y contrapartida +0.01 =>V=0. No cargarlo a venta ni ocultarlo como redondeo normal.                                                                                                                                           |
| V08 | Legado R(2,?), S(1), R(1,4), S(1); tarde c=1                             | Pending antes; replay DEC1: C1=1, V intermedio5/Q2; C2=2.500000000000000000, final Q1,V=2.500000000000000000, ΣC=3.5, entradas6, B=0. Sin duplicar Q.                                                                                                                                           |
| V09 | R(2,10),S(1),R(1,20),S(1); corregir primera c=12                         | C originales10/15, Q1,V15; objetivos12/16,Q1,V16. Deltas entrada+4, salidas−2/−1, netoV+1, B=0. Replay idéntico agrega0; snapshots originales siguen10/15.                                                                                                                                      |
| V10 | Q1,V10; S(2) STRICT; repetir ALLOW_NEGATIVE sin permiso                  | Ambas deniegan; Q1,V10 y cero efectos; desconocer costo tampoco salta STRICT.                                                                                                                                                                                                                   |
| V11 | Q2,V20; S(3),R(0.5,14),R(1.5,12), permiso negativo                       | Q,V: (−1,−10)→(−0.5,−5)→(1,12); C provisional30, ajustes−2/−1, C revisado33, B=0. Sin base, Q0,S1,R(1,7): pending hasta resolución, primer C7 y Q=V=0, no provisional0.                                                                                                                         |
| V12 | Q0,base V10/Q1; S(2),R(1,12),S(1),R(3,15)                                | Q,V: (−2,−20)→(−1,−10)→(−2,−20)→(1,15); ajustes−2/−10, C revisado42, entradas57, B=0. Deuda intercalada sin promedio negativo.                                                                                                                                                                  |
| V13 | Q3,V1; S(1) y R(1,1) concurrentes                                        | S primero: Q3,V=1.666666666666666667, mostrado0.555555555556; R primero: Q3,V=1.500000000000000000, mostrado0.500000000000. CAS perdedor conflicto sin efectos; reintento autorizado con versión nueva recalcula, aunque timestamps coincidan/inviertan.                                        |
| V14 | Apertura0; A:+2→2,c10; B:−1→1; C:+1→2,c20; fechas C<B<A                  | Única cadena A,B,C: COGS B10, Q2,V30, mostrado15.000000000000, B=0; índice derivado no scope_sequence Java.                                                                                                                                                                                     |
| V15 | Apertura0; S1:−1→−1; R1:+1→0,c?; S2:−1→−1; R2:+1→0,c20                   | Válidas S1,R1,S2,R2 y S1,R2,S2,R1; S1 puede costar desconocido o20. Incluso tarde R1=10, S1 puede costar10/20. Qfinal0 y ΣC30 al conocer ambos no confirman snapshots: valor histórico fail-closed, no elegir fecha/UUID.                                                                       |
| V16 | Q2,V20; S(3) => Q−1,V−10,D1,G10; intentar RESELLABLE(1), después R(1,12) | Rechazar retorno sin efectos: Q−1,V−10,D1,G10, elegibilidad/ledger/dedup/secuencia/caja/refund intactos; replay del intento no muta. Receipt posterior: entrada+12, A10,T12, ajuste−2 => Q0,V0,D0,G0,C revisado32 (no V10). QUARANTINE separada, solo según flujo aprobado, no toca pool/deuda. |

## 9. Impactos, pruebas y cambios normativos requeridos (NO realizados)

SQLite STRICT: TEXT decimal checked para operandos/V/deltas; INTEGER para minor,
B y secuencia. PG: NUMERIC(19,6)/(19,4)/(38,10)/(76,18), validación ANTES de insert;
resto entero NUMERIC(19,0), no división SQL aproximada ni NUMERIC(512,0). Ambos
conservan trace/followup; Central recibe snapshots/DEC1/revisiones, no recostea con
policy actual ni usa FK cross-DB. Solo followup nuevo propuesto, no cost_evidence,
causal_order_evidence, value_adjustment_evidence y valuation_resolution separados.
Fundamento consultado: [SQLite STRICT](https://www.sqlite.org/stricttables.html) y
[PostgreSQL 17 NUMERIC](https://www.postgresql.org/docs/17/datatype-numeric.html).
Free mantiene pending físico; overflow/cota no lo habilitan como vía de escape.
Java/Flyway V1–V25 y SQLx publicado intactos. Evidencia obligatoria nueva es diferencia
de producto; campos/errores visibles exigen OpenAPI/ApiError antes de implementar.

Pruebas futuras: V01–16, rechazo RESELLABLE afectado sin efectos y receipt posterior;
policy retirada/sin sucesora: PENDING sin importes con par NULL, actor/cmd/seq/audit;
rechazar NULL parcial, importe/RESOLUTION sin policy; trace/ties, conservación y cota
frente a oráculo racional temporal en secuencias largas; agotamiento sin ajuste,
retornos/deudas con error heredado, límite B y overflow76/128/carry; round-trip SQLite↔PG,
fuente sin pérdida, replay tardío DEC1 y CAS/rollback en cada efecto/dedup divergente;
grafos único/ninguno/múltiples/presupuesto, RBAC/scope/STRICT, offline red denegada,
restore y reportes «entonces/revisado». Mantener PAR/AS/DDL13/`--require-complete`.

| Documento/sección exacta a cambiar por titular tras decidir                                                                                                                            | Cambio normativo solicitado P                                                                                                                                                                                              |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F4-13 §3–4, §7–10, §13                                                                                                                                                                 | MWAC-DEC1, escala18/HALF_EVEN/cotas/umbral, agotamiento exacto registrado vs costo ideal cuantizado; pending≠provisional; déficit/regularización.                                                                          |
| F4-15 §2, §4 Sale/PO/Receipt/StockCount, §5–6                                                                                                                                          | Pactado opcional, evidencia nueva, pending atómico aun sin policy aplicable; RESELLABLE afectado diferido/denegado, QUARANTINE según flujo aprobado, refund separado; déficit solo venta autorizada, secuencia/CAS/scopes. |
| F1-10 §2–3/6–7/10, §16.14–18/21–23/40–41                                                                                                                                               | NULL conjuntos PO/efectivo legado; policy+contrato NULL juntos solo PENDING físico NO_APPLICABLE_POLICY sin importes; V/deltas(76,18), followup/FK/trace/snapshots/secuencia; sin default CONFIRMED.                       |
| F1-12 §2 compras/inventory_movements/branch_inventory/sale snapshots, §4/6 PAR-03/04/06/07/09                                                                                          | 47/47, fuente exacta vs cálculo DEC1 nuevo, money fail-fast, orden ambiguo/derivado, receipts no reejecutadas y diferencias de producto.                                                                                   |
| [F1-11](https://docs.google.com/document/d/1PCgbE95Qt3GjIfy78rPt-tCMjQ2Ed2GwBzEcTfa4HMU/export?format=txt) §4 LOC-05/10/11/13/14/15/17, §6 T-005/010/011/019, §7 DDL-01/03/04/05/12/13 | Pruebas DEC1, cotas/overflow/trace, followup/pending/causalidad; ampliar sin relajar gates.                                                                                                                                |
| F4-21 §1 Inventario/Compras/Ventas, §2 Money/Costeo/Writer, §5–7                                                                                                                       | Referenciar revisión aprobada/excepciones; offline/single writer e import/cutover condicionado a gates intactos.                                                                                                           |
| ADR-0021 §3.1–3.2/5–6; CC21 §2 CC21-03/04/05, §3 D03–D06/4                                                                                                                             | Registrar selección/rechazos y revisión humana; D02 refiere este contrato sin cálculo paralelo. No aceptar D01/D02/D07 por arrastre.                                                                                       |

## 10. Negocio y aceptación pendientes, separados de elecciones técnicas

1. ¿Titular acepta B+D, evidencia/atestación obligatoria para receipt nueva frente a
   Java y responsables habilitados para atestar costo/gratuidad?
2. ¿Acepta continuidad
   física pending sin margen confirmado y déficit de venta con
   doble guard, sin extender permiso a merma/conteo negativo?
3. ¿Responsable contable acepta DEC1 cuantizado/cota propuesta
   , liquidación causal y
   contrapartida de residuo excepcional, sin reescribir margen «conocido entonces»?
4. ¿Quién autoriza apertura económica futura si orden antiguo no puede
   acreditarse,
   reconociendo que no certifica COGS/snapshots ambiguos?

Decisiones técnicas P están concretadas, no «por definir». Aprobador/fecha/revisiones
canónicas/digests/revisión independiente siguen **PENDIENTES**. Validación documental/
aritmética sintética no acredita motores, integración, import, CI ni aceptación.
