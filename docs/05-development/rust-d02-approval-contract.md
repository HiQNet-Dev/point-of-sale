# D02 — contrato final propuesto para aprobación de catálogos (ADR-0021)

**PROPUESTO / NO ADOPTADO · 2026-10-28.** No existe aceptación nueva ni aprobador atribuido.
Rama verificada: `feat/backend/rust-0d-readiness`; base/HEAD `329c0984722e8339f3e3b29497960d426ac2c43c`, árbol inicialmente limpio.
Write set exclusivo: este archivo nuevo. Sin DDL, código, contratos HTTP, cambios canónicos, import, cutover ni commits.
Este anexo cierra **técnicamente como recomendación** todas las Q de [la propuesta candidata](rust-missing-physical-proposal.md) §5–9.
No cierra unilateralmente [ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md), D01/D03–D07 ni los gates de [CC21](rust-adr0021-change-control.md).

## 1. Autoridad y evidencia reconsultada

**N** = obligación explícita del baseline F1-10/F1-12 o ratificada por F4-21; no significa ley fiscal ni implementación.
**P** = elección recomendada aquí, **requiere aprobación**, incluso cuando concreta un antecedente candidato C; nunca se atribuye a una fuente que no la decide.
**R** = dato real que el titular debe configurar/acreditar; no es una pregunta técnica pendiente ni un valor que pueda inventar el agente.
Se heredan los diccionarios candidatos §4–9 por referencia; §2–7 especifican las diferencias exhaustivas de esta recomendación, no cinco fichas duplicadas.
Si hay conflicto, N permanece vigente y P se tramita mediante CC21; este documento no sustituye el freeze.

Consulta actual: exports TXT y dos XLSX leídos en memoria, sin credenciales adicionales ni copias del expediente en el checkout.
URLs mutables: SHA-256 identifica bytes recibidos sin normalizar BOM/saltos; no identifica revisión inmutable ni aprobación.
Las referencias de esta tabla son las URLs de export efectivamente consultadas; los documentos completos no se reproducen.

| Fuente                                                                                                                      | Secciones/uso                                                             | SHA-256 actual                                                     |
| --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| [F1-10](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/export?format=txt)                  | §1–3,16.1,16.10,16.18–19,16.37–41,16.60–62: tipos, FK, versiones, brechas | `8a3354af4c1634ad0c97ebccb8be787202e67423d37cc5df548d4137e4549e5f` |
| [F1-12](https://docs.google.com/document/d/16hyT4XALY3DrAc50RUTx-fESzMI0c5612zTKphHRieM/export?format=txt)                  | §2,4,6 y freeze: 37/37, UUID, snapshots, NULL, escala                     | `e0dfa203c4c4ca5bc66b3b4ea3a9e7cced017ef300a9367295836128a5af5729` |
| [F4-21](https://docs.google.com/document/d/1tIHbbRw5GhKOFkL4ff0Zoc-SmMc-q7J2WwyYXFjqiUE/export?format=txt)                  | §1–3,6–7: autoridad, Free, ratificación y control                         | `b1912c884ec384b62944ca366810eaf809fcd257e6bde85744d5c15e7978548d` |
| [F1-08](https://docs.google.com/document/d/1T4yX0nRPN4dGKJKsyTbwemkH3EZrTzOT1cUbF6XDEJU/export?format=txt)                  | §6–9,16–17: ownership y relaciones candidatas                             | `de5958d5105b636249b97b81b3a1c85bc1dac85149a8670d08cf3a0acba76a63` |
| [F1-09](https://docs.google.com/document/d/1aPZZcWmf3GvoNFfsLnpESg_Rym9V49-1DcTMwoxsvnw/export?format=txt)                  | §1–4,6–11,14: diccionario candidato y acceso al libro integral            | `40346b174cf811d4843368655899ba843c53f54a30804ae51cb01bd0a1dab484` |
| [XLSX integral F1-09](https://drive.google.com/uc?export=download&id=14WObt9M5AIU-tCqUUuQMxh-X2dqZzKr3)                     | Entidades 31,37–39,50,55,77; Campos 160–163,196–210,260–264,286–289       | `899389e876835f10bc79b905494f1a4fff8b25dcc9cdb1044297ebae85da9462` |
| [Matriz XLSX F1-10](https://docs.google.com/spreadsheets/d/1ruR5jU38WZFwwv_tzQay2Y28CqLYOfpbYSU7m4hyHD4/export?format=xlsx) | Entidades 31,37–39,45,55,79: nombres/scopes todavía candidatos            | `8d91aa1c9968ed18f2ea3cf59859921b880f08664217ddb44dff7b1c96193e69` |
| [F4-13](https://docs.google.com/document/d/1vL-yFrwenxeqbAjuWY9m5zOMhlMGADfVTDUeWaOa4OM/export?format=txt)                  | §3–10,13 y ratificaciones: Money/Tax/MWAC, residuos y negativos           | `2574340697c13d20b98f1188e706cae2ccf2147c37a737806d58d7788be96cc2` |
| [F4-15](https://docs.google.com/document/d/12VYpuy-IznZufJpYiGqn-4BBi3JqeI0al3XCE5Gx7tU/export?format=txt)                  | §2,4–6: lifecycle, scope y transacciones                                  | `0eac0adb025c6865c87612896d92cbcb1f43318fc141a871c28b439415d8e6f4` |
| [F4-16](https://docs.google.com/document/d/1zI_TjkKOG7iBTT1sYe5223yZxBsVfi0B4CSZE6u9efg/export?format=txt)                  | §2–6,8: grants, Free, protección core, downgrade                          | `2ee493c06ef3fc6f0ef484b3b0e72aff466ebc095335b34c4b1cd7e1a0d659d2` |
| [F4-17](https://docs.google.com/document/d/1eUt4MmWDEzpCVuj6LmUBTD4zEvveH9lOMRbaBcr2hsU/export?format=txt)                  | §1,3–4,8–9: precisión, rounding explícito, frontera CFDI                  | `7adf62fb8b34d55bb92ac675d4c868e6e39dfee39b61629957675ca716225766` |
| [F5-09](https://docs.google.com/document/d/1_Fo4JkP546ucsJA6wiX6u8OK3lHSfScVQ-AoOKu4Ovo/export?format=txt)                  | §2,5–6,9–13: categoría, devengo/pago, no stock automático                 | `cc37a9c23e41e43374a0da6b0842215082612698addc1a14cc3cbff417dc9e75` |
| [F5-10](https://docs.google.com/document/d/1AHq2wAbf4vY2U0oMW9bqw8F_Wdu-zugqkbYhHQDqs80/export?format=txt)                  | §2–4,6–10,13: catálogo org, selección fiscal, MWAC, historia              | `06b2bd8607efa036575d99f3972b6e64d4aff87590e904bbc891d265a66c9c62` |
| [F5-12](https://docs.google.com/document/d/1QYer540xq2861EU5peA5aePLvbk-9dIRXemcI6Z71Zo/export?format=txt)                  | §2–5,7,11,13,15: Plan global candidato, roles CP y evidencia              | `861d2c482adf7792d33405f29b1d79976325be3a836b55caf598dc2369c09947` |
| [F5-13](https://docs.google.com/document/d/12wzZDsw-m8t0Fn8NOvniFQhvx-tAtO-4kxMnP4GH4uQ/export?format=txt)                  | §1,3–7,9–12: límites/ediciones; Pro es hipótesis, no tarifa               | `c72cea808f868ef90c26b841b183e4811d1890474a093d3fd3e38d286edab49d` |
| [F1-11](https://docs.google.com/document/d/1PCgbE95Qt3GjIfy78rPt-tCMjQ2Ed2GwBzEcTfa4HMU/export?format=txt)                  | §4–7: integridad, publicación y gates                                     | `6f39fd5a2ba9b3a55575bbf2e00390f0a066345eb309b55bbd8ba37700b8460d` |
| [F1-13](https://docs.google.com/document/d/1ZtDnxOeVjeBJD20gLI1GxA4cTY-bevRoL2fKUMqZBPk/export?format=txt)                  | Plan DDL13/PAR/AS reconsultado; no ejecución                              | `cabb799eeb5991046cb07e4e2983cffed2507dae80bf809719964162d29cf7a3` |

Los TXT y el XLSX integral coinciden con 0c. Matriz F1-10: 19 017 bytes hoy frente a 19 032/hash `43c54e3623dcbe72c72dfec66cf1ad46cb4bc3d0020dd7e619e667d4dc37417e` en 0c.
Las filas focales consultadas mantienen las ambigüedades; no se afirma igualdad del libro completo ni causa de la diferencia de export.
Referencias locales fijadas en la base: candidato SHA-256 `6ab9b372962ef3de4525da21baed6e6ce040caf2b5621d414f095e9e306070b5`; CC21 `8a22dad54575fd664ac7c9d61c516e48f0707a495b7a189756f0d7233239e9b6`.
ADR21 SHA-256 `70f56f212858fef38861e725f2161e28d547e8cca224234acab605d1c51ef29f`; [contrato de compras](rust-purchase-parity-contract.md) `6f389b3c8299a947ef1e6775e2021baedd5805ff541050cd9deb4ea5f9dec078`.
Integración D03–D06: [anexo único de valoración](rust-valuation-approval-contract.md), **P MWAC-DEC1**; referencias estables por sección: §3 evidencia/representación, §4 continuidad D05, §5–6 contrato D06/regularización, §7 causalidad y §8–9 vectores/impactos. No se fija un hash mutable de ese anexo de trabajo ni se adopta su propuesta.
No se consultaron comentarios/revisiones privados, datos productivos, FigJam ni legislación SAT actual; las fuentes comerciales siguen siendo propuestas donde F4-21 no las ratifica.

## 2. Contrato común P: validación, ownership y diferencias respecto del candidato

Se conservan todos los campos, NULL/defaults, PK/UK/FK y cardinalidades de §4–9 del candidato salvo diferencias enumeradas aquí y en §3–7.
Ownership recomendado: PriceBook/TaxProfile/ExpenseCategory de Organization; CostingPolicy de Branch, aplicada separadamente por item; Plan global de Control Plane.
Un writer administrativo local designado por scope; ser dueño del UUID no autoriza a escribir. Deny-by-default, usuario/organización activos, permiso efectivo y ámbito comprobados en cada comando.
**P:** autorizaciones funcionales separadas para editar catálogo, publicar precio, publicar configuración fiscal, publicar costeo y administrar categorías; no son claves RBAC ya existentes.
Un rol branch no modifica catálogo org sin concesión org expresa; cajero no ve costo/margen; operador de Control Plane no obtiene permisos POS. Publicar condiciones comerciales exige autorización de Control Plane separada (§7).
Mutación = CAS `expectedVersion`, commandId/digest, validación y audit/event/outbox aplicable en una transacción. Mismo comando/payload retorna resultado; distinto payload es conflicto.

| Diferencia exacta sobre candidato §4 | Decisión P (no cifra normativa)                                                                                                                                                                                      |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Longitudes antes Q                   | `code`: 1–64 ASCII, trim + uppercase, patrón `[A-Z0-9][A-Z0-9_-]{0,63}`; `name`: 1–120 escalares Unicode tras trim/NFC; rechazar controles/NUL. No truncar.                                                          |
| Otras claves/configuración           | `jurisdiction`: 2–32 ASCII `[A-Z0-9][A-Z0-9_-]*`, catálogo local explícito; `tax_code`: mismo contrato que code; identificadores de contrato/policy: 1–64 `[A-Z0-9][A-Z0-9_.-]*`.                                    |
| Tipos/tiempo concretados             | UUID TEXT minúsculo 36 canónico / UUID PG; UTC TEXT fijo `YYYY-MM-DDTHH:mm:ss.ffffffZ` / TIMESTAMPTZ(6), años 0001–9999. Rechazar precisión histórica no representable, nunca truncarla.                             |
| Versiones/moneda concretadas         | CAS 0..2^63−1, revisión numérica 1..2^63−1 checked; PlanVersion conserva TEXT (§7). Currency: 3 letras mayúsculas en catálogo ISO-4217 versionado incluido localmente con minor digits, sin default MXN.             |
| Normalización/enforcement            | Normalizar solo nuevas etiquetas/códigos; no reescribir snapshots legados. Misma validación Rust en ambos motores + CHECK de forma/longitud y UK sobre valor normalizado; no depender de collations distintas.       |
| Lifecycle/retención concretados      | Padres ACTIVE↔INACTIVE con CAS; código/ID/scope/moneda no reasignables. Sin hard delete operativo ni purga automática, aun sin referencias; retención legal futura separada. RESTRICT en todas FK.                   |
| Índices antes condicionales          | Adoptar los UK/FK del candidato y los índices de resolución de §3–7; omitir índices de acceso solo-status y el de auditoría de CostingPolicy propuesto sobre `(costing_policy_id,created_at)` hasta consulta medida. |

N: F4-15 §5 requiere claves scoped. P: UK `(organization_id,id)` en padres org; `(organization_id,branch_id,id)` en política; FK hacia Organization y Branch del mismo org.
Completar `organization_id` en PriceVersion/TaxRuleVersion y `organization_id,branch_id` en ValuationEntry; FK compuestas a padres/producto/movimiento; sus propias UK scoped soportan hijos.
Índice de cada FK hija por sus columnas salvo cobertura por prefijo de UK/índice de resolución; PriceVersion se agrega abajo con org al índice candidato. Expense usa el índice §6.
SQLite: STRICT y foreign_keys=ON comprobado por conexión antes de TX; BEGIN IMMEDIATE para guards. PG: tipos nativos y lock del padre/scope antes de guard; nunca FK cross-DB.
No se promete equivalencia por tener tipos parecidos: pruebas negativas de escritura directa y de dominio separarán CHECK/FK, triggers de inmutabilidad, permisos y guards multi-fila.

**P: sucesión común de versiones**, para precios, reglas fiscales, asignaciones y costeo: DRAFT→PUBLISHED→RETIRED, o PUBLISHED→CANCELLED_BEFORE_EFFECTIVE; terminales sin retorno. Borrador editable, identidad/scope estables.
Añadir donde falten `status`, `published_at`, `retired_at`, `cancelled_at`, `row_version`, `created_at`, `updated_at`: tipos comunes; tiempos de transición nullable/default NULL hasta su transición, sin default de publicación; status DRAFT y CAS 0.
Precio/regla/asignación: mantener `effective_from`, `effective_to` opcional; **CostingPolicy mantiene solo `effective_from`**, sin `effective_to`. Intervalo efectivo `[from,min(to declarado,next_from,retired_at))`, omitiendo extremos ausentes.
Cronología efectiva ordenada por inicio: solo versiones publicadas **no canceladas**, incluidas RETIRED; `next_from` es su siguiente inicio. CANCELLED_BEFORE_EFFECTIVE es tombstone auditable, excluido de selección, fronteras y unicidad de inicios efectivos; no reserva hueco.
Payload/inicio/to publicado quedan congelados; retiro efectivo solo si ahora >= inicio, fija retired_at=ahora y auditoría, nunca simula cancelación futura. Retirada efectiva sin sucesor/hueco explícito no permite fallback a versión vieja ni borrar frontera histórica.
Revisión estrictamente creciente por secuencia, contando también las canceladas; **inicio no monotónico por revisión**: nueva versión puede insertarse con inicio >= ahora, distinto de cualquier inicio no cancelado, sin cambiar intervalos anteriores a ahora ni hechos committed. `to>from` si existe; intervalo efectivo no vacío al publicar.
Sustituir UK permanente de inicio de CostingPolicy por unicidad de inicio para PUBLISHED/RETIRED no canceladas; aplicar igual guard/índice parcial a otras secuencias cuando corresponda. La UK de revisión no excluye canceladas ni permite reutilizar revisión/ID.
Cancelación solo con permiso/CAS, ahora < inicio y sin hechos committed que la usen; fija cancelled_at, conserva payload/revisión/auditoría, no retired_at. Cancelar dependencias fiscales futuras o reemplazarlas en batch válido; no dejar APPLY apuntando a regla cancelada.
Una cancelación puede extender hacia el futuro el predecesor aún vigente, nunca sobrepasar su to/retiro explícito ni alterar pasado. Permite publicar reemplazo inmediato o contiguo con revisión mayor, aunque su inicio sea anterior al del tombstone; quotes previos se invalidan.
Publicación/cancelación/retiro serializan el scope y revalidan cronología/configuración antes de commit; snapshots conservan IDs usados. Para STOP rige además §4.1: no caducidad/retiro efectivo, sí cancelación futura sin efecto.
Checkout revalida precio/impuestos y digest de configuración en la TX; cambio desde quote exige recotizar/consentimiento explícito, aunque el total coincida si cambió clasificación. Sin cobro silencioso.
Import inicial es vía separada, procedencia y manifest explícitos, nunca exención genérica de invariantes: config válida desde cutover, no supuesta vigente en fechas de ventas legadas.

## 3. Q-PB — precio, moneda, selección y bootstrap

**N:** F1-10 §16.37 exige padre y versión inmutable; F1-12 conserva precio Java exacto. **P:** ratificar candidato §5 org-only, sin branch overrides ni conversión FX en v1.
Permitir 0..N libros históricos, máximo uno ACTIVE por org mediante guard de activación TX, no UK permanente por org. Para vender debe existir exactamente uno y cubrir producto/moneda/instante.
Activar otro libro desactiva el anterior atómicamente con ambas CAS; sin libro/precio vigente no se cotiza ese producto ni se cobra carrito que lo contenga; no usar `Product.unit_price` como fallback.
Currency del libro = Organization al activarlo y al cotizar. Recomendar **rechazar cambio de moneda de Organization después de cualquier precio/hecho monetario**, incluso borrador; nueva org o futura migración FX revisada.
Antes de existir tales datos se admite cambiar moneda con CAS; libros inactivos de otra moneda se conservan y no son activables. No reinterpretar Money histórico ni cambiar moneda del libro.
**Delta §5:** confirmar code/name/límites §2; añadir lifecycle al hijo según §2; índice `(organization_id,price_book_id,product_id,effective_from)` y UK `(organization_id,price_book_id,product_id,version)`.
FK hijo→Book/Product con org, currency validada contra libro. Secuencia temporal por org/libro/producto; dos productos no comparten contador. `tax_mode` físico INCLUSIVE/EXCLUSIVE, aliases GROSS/NET solo de dominio.
**R:** IDs persistidos de libro/versiones, code/name, moneda, instante de cutover y modo fiscal de precios Java deben figurar en manifest versionado con fuente/digest y aprobación del titular.
El precio fuente se conserva (no se convierte NET↔GROSS sin evidencia). Falta modo/clasificación: preservar catálogo importado, producto no habilitado para nueva venta hasta configurar, sin inventar régimen histórico.
Retry del manifest usa mismos IDs/digest y no duplica; payload nuevo no sobrescribe versión publicada. La versión inicial vale desde cutover; ventas antiguas conservan snapshots y price_version_id NULL cuando no existía.

## 4. Q-TP — selección fiscal y multiimpuestos, sin motor arbitrario

**N:** F4-13/F4-17 ratificados exigen NET/GROSS, clasificación explícita, rounding versionado y snapshots; no aprueban una tasa ni HALF_UP universal.
**P:** un TaxProfile representa **un componente tributario** org/jurisdicción/tax_code/direction, no un conjunto opaco. Una TaxRuleVersion es su tratamiento/tasa en una revisión.
Organization 1:N perfiles; perfil 1:N reglas; producto 0:N asignaciones; línea nueva 1:N snapshots, incluidos componentes explícitos de importe cero. Perfil no es único por org/jurisdicción.
**Delta §6:** añadir al padre `tax_code` TEXT NN, `direction` TEXT NN TRANSFER/WITHHOLDING, `calculation_contract_version` y `rounding_policy_id` TEXT NN (contrato §2), todos sin default e inmutables desde alta junto con jurisdicción/valid_from.
Elegir explícitamente TAX_SIMPLE_1/POS_LINE_HE_1; todos los componentes de la línea deben concordar en ambos IDs, resolubles por registro confiable de release. No elegir policy por locale ni usar la última del runtime. No tabla de conjunto adicional.
Regla: conservar classification/rate_exact, añadir lifecycle §2; UK `(organization_id,tax_profile_id,version)`, índice `(organization_id,tax_profile_id,effective_from)`; no UK que permita una sola regla de todos los impuestos.
Clasificación P: TAXABLE, ZERO_RATE, EXEMPT, NOT_SUBJECT; TAXABLE tasa >0; ZERO_RATE tasa =0; EXEMPT/NOT_SUBJECT tasa almacenada 0 como neutral técnico, jamás inferir clasificación desde ese cero.
Catálogo de jurisdicción/configuración es local, versionado y autorizado por titular con revisión fiscal aplicable; Branch resuelve **una jurisdicción efectiva** explícita por comando, sin inferir por locale/IP/moneda.
**Delta adicional:** `branch.tax_jurisdiction` TEXT nullable, default NULL para ausencia legada, obligatorio antes de nueva venta (contrato §2); histórico sin dato se conserva con procedencia, no se backfillea MX. Requiere ficha reconciliada.
Instante del comando fijado por backend UTC; vigencias por fecha local se convierten usando timezone IANA de Branch, exigiendo offset explícito ante DST ambiguo y rechazando hora inexistente. Congelar instante/zona/config en snapshot.
ProductTaxAssignment de F1-08 §16 dice `tax_rule_id`; XLSX Campos 207–210 precisa `tax_rule_version_id`. Se recomienda este último enlace **a versión concreta para APPLY**, no “última regla del perfil”; STOP no aplica regla.

### 4.1 Relación adicional P `product_tax_assignment` — contrato revisable suficiente

No está definida como ficha en el físico contado: **añade una relación local**, no la materializan los cinco padres. Se exige su ficha propia y aprobación en CC21-02.

| Campos añadidos/reutilizados                                                                                 | Tipo Local / PG, NULL/default                                         | Claves y semántica P                                                                                                                             |
| ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `id`, `organization_id`, `product_id`                                                                        | UUID TEXT / UUID; NN, sin default                                     | PK id; UK(org,id); FK(org,product) RESTRICT.                                                                                                     |
| `operation`, `jurisdiction`, `tax_code`, `direction`                                                         | TEXT / TEXT NN, sin default, contratos §2/4                           | Clave física tipada de slot junto con org/product; inmutable. Operation solo SALE; direction TRANSFER/WITHHOLDING. Refund usa snapshot original. |
| `effect`, `tax_rule_version_id`                                                                              | TEXT NN APPLY/STOP sin default; UUID TEXT/UUID nullable, default NULL | CHECK: APPLY exige rule NN; STOP exige rule NULL. FK compuesta a regla y slot RESTRICT cuando APPLY. Sin regla ficticia para STOP.               |
| `effective_from`, `effective_to`                                                                             | UTC / TIMESTAMPTZ(6); inicio NN sin default; fin nullable NULL        | APPLY dentro de vigencia de regla/perfil; STOP exige fin NULL. Sucesión/cancelación §2; sin habilitación histórica por import.                   |
| `version`, `status`, `published_at`, `retired_at`, `cancelled_at`, `row_version`, `created_at`, `updated_at` | INTEGER/BIGINT y tipos §2                                             | Version >=1 sin default; lifecycle §2; revisión por slot, no por producto completo.                                                              |

Slot físico `K=(organization_id,product_id,operation,jurisdiction,tax_code,direction)`; UK(K,version) incluye canceladas, unicidad de (K,effective_from) solo PUBLISHED/RETIRED no canceladas. Índice de lookup (K,effective_from); guard TX bloquea producto para validar slots/conjunto.
**Delta de integridad:** TaxRuleVersion añade jurisdiction/tax_code/direction NN sin default, copia inmutable de su perfil; UK(org,id,jurisdiction,tax_code,direction) y FK(org,profile,jurisdiction,tax_code,direction) a UK homóloga del perfil. APPLY usa FK(org,rule,jurisdiction,tax_code,direction) a esa regla; STOP conserva K aun sin regla. Índices FK por prefijo no cubierto.
Solo publicar STOP autorizado sobre un slot conocido cuyo último evento no cancelado al inicio solicitado sea APPLY, aun vencido; exige motivo auditado. STOP no afirma ZERO_RATE/EXEMPT/NOT_SUBJECT, no genera tasa/snapshot fiscal y no verifica perfil activo para terminarlo.
STOP publicado persiste hasta un APPLY posterior explícito: effective_to y retired_at NULL obligatorios, sin retiro efectivo; puede cancelarse solo antes de inicio según §2. Repetir command/digest es no-op; otro STOP consecutivo se rechaza. Reabrir exige APPLY con regla válida y revisión mayor.
Resolución por producto+SALE+jurisdicción+instante: por cada K conocido elegir evento publicado no cancelado de mayor inicio <= instante, antes de filtrar ventanas/reglas. STOP omite legítimamente ese componente; APPLY debe estar vigente con regla/perfil activos/habilitados, de otro modo error. Nunca fallback ni omisión silenciosa de APPLY inválido.
Una regla sucesora sin reasignación también falla; publicar/cancelar reglas y APPLY/STOP afectados en **batch autorizado**, validando conjunto completo en cada frontera futura afectada y al cotizar. Revision crece por K; fechas siguen §2, no la revisión.
Resultado de APPLY no vacío, máximo **8 componentes**, sin duplicado tax_code/direction; NOT_SUBJECT, cuando corresponda realmente, es **único APPLY**. Terminar todos sin reemplazo fiscal explícito es configuración incompleta, no exención por defecto.
EXEMPT/ZERO_RATE no coexisten con TAXABLE del mismo slot; tributos distintos pueden coexistir. Para pasar a NOT_SUBJECT, mismo batch STOP de todos los otros APPLY + APPLY NOT_SUBJECT legítimo; para volver a gravado, STOP de NOT_SUBJECT + APPLY requeridos. Sin tasa inventada ni segunda relación.

### 4.2 Cálculo y límites de configuración P

Recomendar contrato `TAX_SIMPLE_1`: porcentajes independientes sobre una misma base descontada, tras deltas de modificadores; clasificación explícita, tasa 0..1 con hasta 12 decimales (`NUMERIC(13,12)` PG).
No cuotas por unidad, impuesto sobre impuesto, bases diferentes por tributo, scripts, reglas arbitrarias ni retenciones con fórmula especial en v1; esas configuraciones se rechazan, no se aproximan con porcentaje.
R: fiscalista/titular determina si ese subconjunto sirve a las operaciones reales, códigos/tasas, dirección, vigencias y modo. Si no sirve, ampliar contrato mediante decisión revisada antes de habilitar esos productos.
**P fiscal**, no normativo: rounding recomendado `POS_LINE_HE_1` con HALF_EVEN explícito; base GROSS dividida racionalmente y cuantizada a 12 decimales; NET conserva base decimal hasta 12; luego cada componente a minor digits de moneda.
Comparte el modo propuesto HALF_EVEN con la selección de valoración MWAC-DEC1, no sus fronteras de cálculo: la política fiscal requiere aprobación propia y no se deduce del contrato de costeo.
NET: final = round(base) + suma(traslados minor) − suma(retenciones minor). GROSS: base = gross/(1+Σtasas transfer−Σtasas withholding), denominador >0; final = round(gross), no sumar impuesto otra vez.
En GROSS, `net_minor=final−Σtransfer_minor+Σwithholding_minor`; diferencia frente a round(base) queda como `rounding_residue_minor` explícito en snapshot de línea, nunca redistribuida a otro producto/exento.
Rechazar net/final negativos, overflow o residuo absoluto > número de componentes minor units; límite de diagnóstico P, no corrección automática. No ofrecer POS_LINE_HE_1 como norma SAT ni preset obligatorio.
Descuento global: reparto por mayor resto sobre bases elegibles, empate por UUID de línea, sin superar base; snapshot del reparto.
**P refund acumulativo:** por línea original, `refund_tramo_minor=HALF_EVEN(total_original_minor×qty_acumulada/qty_original)−refunded_minor`; qty acumulada solo de devoluciones aceptadas/reservadas del mismo origen, 0..qty_original, sin redondear cantidades ni cada tramo por separado. `refunded_minor` incluye importes ya asignados/reservados para esa cantidad, no solo pagos desembolsados; CAS/lock serializa ambos acumulados y reservas. Retry no avanza acumulado ni paga dos veces.
Ejemplo: 5 unidades por 0.03, cinco devoluciones unitarias => **1,0,1,0,1 centavos**, acumulados 1,1,2,2,3. No usar HE(0.006) independiente ni “último tramo absorbe”: al completar qty, total reembolsado coincide exactamente con el original.
Desglose fiscal desde snapshot original, sin reglas actuales: objetivo acumulado de cada traslado/retención = HE(importe_original×qty_acumulada/qty_original), acotado entre ya asignado y original. Neto acumulado = objetivo total−traslados+retenciones.
Si neto cae fuera de [neto_ya_asignado,neto_original], reconciliar determinísticamente hasta el límite más cercano: para subir neto reducir traslados y luego aumentar retenciones; para bajarlo aumentar traslados y luego reducir retenciones, siempre dentro de sus cotas y por tax_code/direction/ID estables. Registrar reparto/residuo, no alterar bases ni clasificación para ocultarlo.
Cada tramo es diferencia de acumulados: neto+traslados−retenciones=refund del tramo, ningún componente retrocede/excede original; al completar, todos coinciden con snapshot original. Conservar bases exactas y proporción del snapshot para trazabilidad, separadas del reparto minor. Legado sin desglose conserva ausencia, no genera detalle fiscal nuevo.
**Delta snapshots:** resolver contradicción F1-10 §16.10/39: retirar FK escalar `sale_line.tax_snapshot_id`, relación autoritativa por hijo.sale_line_id 1:N; quitar UK de ese campo hijo si significaba 1:1.
Añadir org/branch NN al snapshot, FK scoped a SaleLine y TaxRuleVersion; `assignment_id` UUID TEXT/UUID y `tax_code_snapshot`, `direction_snapshot` TEXT nullable/default NULL por legado, obligatorios para nuevo cálculo; UK(org,branch,line,rule) para nuevos.
Snapshots nuevos guardan rule/assignment no nulos, clasificación/base/tasa/currency, modo, policy ID, versión/digest de contrato e inputs/resultados; `snapshot_json` versionado admite esos escalares, nunca sustituye las FK.
Usar `sale_line.snapshot_json` existente para net_minor/residuo y reparto; no sumar esa base repetidamente por cada tributo. Rounding IDs resuelven registro de contratos confiable incluido en release, no scripts o tabla dinámica obligatoria.
Legacy sin detalle fiscal: conservar snapshots/importes originales y ausencia de rule/assignment; marcar `LEGACY_UNRECORDED` en envelope versionado, no crear filas EXEMPT/0 inventadas ni desglose reconstruido desde configuración actual.
Quote con config incompleta/ambigua/contrato desconocido falla antes de efectos; carrito puede guardarse, solo checkout afectado se bloquea. Reimpresión/refund histórico conserva ausencia y no se anuncia fiscalmente completo.
**Alternativa minimalista revisable:** máximo un componente porcentual por producto, misma relación explícita y lifecycle; rechazar multiimpuestos. Reduce cálculo/tests, no elimina la brecha de asignación. No recomendada para cubrir multiimpuestos reales ni declarable como alcance completo sin aprobación.

## 5. Q-CP — ownership y lifecycle de CostingPolicy; valoración por referencia

**N:** MWAC por Branch/InventoryItem, STRICT normal, ledger físico/económico separados append-only (F4-13 §7–10 y ratificación). La representación de pendientes y continuidad D05 sigue siendo P.
**P:** ratificar §7 candidato sin item_id ni override: una secuencia de política por Branch, aplicada con saldos aislados por item/unidad; método único MOVING_WEIGHTED_AVERAGE.
**Delta §7:** `calculation_contract_version=MWAC-DEC1` explícito al publicar, sin default SQL; published_at/retired_at y CAS según §2. Conservar UK scoped de revisión/inicio y FK compuesta desde ValuationEntry a política/movimiento.
Lifecycle y cancelación futura según §2; publicación congela scope/método/contrato/inicio, sucesión no retroactiva. Retiro efectivo conserva referencias y no reactiva predecesor; tombstone futuro no reserva frontera. No impone por sí mismo otro bloqueo a continuidad física pending.
**Única propuesta D03–D06:** [anexo de valoración](rust-valuation-approval-contract.md) §3–9, contrato propuesto MWAC-DEC1. Allí se definen escalas/rangos, costo de salida, agotamiento, promedio solo de presentación, evidencia, seguimiento, resoluciones, causalidad y vectores; D02 no reproduce fórmulas, tablas auxiliares ni otro algoritmo.
Q-CP.2 remite a su D06 (§5–6/8–9); Q-CP.3 a D03–D04 (§3); Q-CP.4 a continuidad D05 (§4), seguimiento/regularización (§3/6) y cutover/causalidad (§7). No quedan variantes numéricas o de followup propias de D02.
En particular, **P D05 permite salida física autorizada sobre legado pending**, respetando stock policy y demás guards, con seguimiento económico atómico y sin costo/margen confirmado ni número inventado; ausencia de base no equivale a PROVISIONAL cero ni deniega por sí sola esa continuidad.
**R:** cutover e IDs/versiones iniciales son configuración explícita; la evidencia económica y su procedencia se rigen por el anexo. Referenciar MWAC-DEC1 desde cutover no afirma que Java ejecutó esa política históricamente ni desbloquea import/cutover.

## 6. Q-EC — categoría estable, no historial nominal inventado

**N:** Expense categoría NN, org y Branch opcional; devengo/pago separados (F1-10 §16.19/F4-15 §5/F5-09 §5–6).
**P:** ratificar §8 candidato org compartido, sin categoría branch, jerarquía ni deducibilidad implícita. Code estable, nombre mutable, nombre repetido permitido.
Elegir **identidad estable + etiqueta actual** para reportes v1, no añadir snapshot/versionado nominal; UI/export debe decir “categoría actual” cuando muestre nombre. No prometer nombre histórico al devengo.
DRAFT admite cambio de category_id a categoría ACTIVE del mismo org con CAS/audit de antes/después; desde POSTED no reclasificar, ni por UPDATE ni por renombre del padre.
Error posterior exige compensación/reemplazo del gasto por flujo autorizado que respete pagos existentes; no crear un mecanismo nuevo de reclasificación silenciosa en D02.
Desactivar impide nuevas asignaciones y POSTED desde DRAFT; no bloquea pago de gasto ya POSTED. CASH produce un egreso idempotente; BANK ninguno, sin compra/stock automático.
**Delta §8:** longitudes §2 e índice elegido `(organization_id,category_id,posted_at)`; FK category compuesta aun si branch NULL; Branch opcional FK(org,branch) cuando presente. Sin otros campos nuevos.
**R:** titular crea catálogo explícito code/name/IDs; no seed “OTROS”, no import 1:1 desde Product.category, no categoría por defecto para satisfacer FK. Sin categoría no se postea.

## 7. Q-PL — catálogo global, aprobación y grants sin adoptar Pro

**N:** padre de PlanVersion, términos publicados inmutables, RBAC separado de entitlements y Free sin CP obligatorio (F1-10 §9/16.60–62, F4-16/F4-21).
**P:** ratificar §9 candidato: Plan global HiQNet, código global §2, sin organization/customer/tenant_id. No plan privado Enterprise en D02: condiciones particulares en futura versión/contrato, sin nuevo owner implícito.
Nombre mutable es etiqueta actual; **delta PlanVersion:** añadir `name_snapshot` TEXT (1–120), `terms_ref` TEXT (1–256 sin controles) y `terms_digest` TEXT SHA-256 hexadecimal minúsculo 64; nullable/default NULL en DRAFT, obligatorios al publicar y luego inmutables.
Terms_ref referencia evidencia contractual versionada, no secreto ni URL a descargar automáticamente; digest del bundle JSON canónico UTF-8 con hash del documento de términos y grants ordenados por key. Cambio exige nueva versión, jamás alterar contrato firmado.
PlanVersion.version sigue TEXT: P representación de entero decimal positivo 1..2^63−1 sin ceros iniciales, no SemVer; UK(plan_id,version), comparación numérica validada, no orden lexicográfico.
INACTIVE impide publicar/contratar nuevos incluso si versión PUBLISHED; no invalida subscription/license ni grants emitidos. Retiro de versión bloquea nueva contratación, no recalcula derechos ya documentados.
Publicación recomendada: preparador comercial CP y aprobador CP distinto con permiso de publicación; registro auditable de actor/revisor/fecha, versión/digest, términos y matriz de grants/tests. Son funciones propuestas, no personas ni aprobaciones existentes.
Permiso POS, pago proveedor o flag de UI no publica ni concede Pro. Sin evidencia completa no publicar; modificar grant/condición requiere nueva PlanVersion y emisión/transición aceptada, no editar padre.
**P límites técnicos de configuración:** catálogo cerrado de keys soportadas por release; BOOLEAN/INTEGER/ENUM, scope/unidad/overflow/enforcement compatibles; enteros 0..2^63−1, NULL solo para ausencia de cap en keys declaradas, ENUM en whitelist.
Máximo 128 keys por versión, key ASCII 1–64 `[a-z][a-z0-9_.]*`, valor individual <=1 KiB JSON canónico, sin scripts/secretos; son límites técnicos propuestos, no cuotas comerciales. Desconocido/tipo inválido impide publicar/activar grant.
Free usa política local confiable versionada incluida en release: branches.active=1, installations.active=1, sync.enabled=false, reporting.central=false, backup/export/rbac/audit=true; sin cap comercial de users/products/sales, soporte SELF_SERVICE.
Esos valores siguen F4-16 §5; no requieren filas Plan/Subscription/License ni firma remota para checkout. Nuevas keys no concedidas => deny de esa capability; no desactivar protecciones core por key omitida/false.
Límites de una lista activa/8 componentes son guards de configuración funcional P, no entitlements Pro. No añadir cuotas de usuarios, ventas, productos o caja concurrente por analogía con F5-13.
Exceso/downgrade: después de safe point, titular selecciona Branch/Installation operativas Free; resto lectura/export/backup. No borrar historial, revocar transacción comprometida ni bloquear core por CP caído/licencia ajena.
**R, fuera de habilitación D02:** precio, impuestos HiQNet, moneda comercial, plazo, cuotas Pro/Enterprise, grace, proveedor/SLA. Se almacenarán en versión/contrato aprobado, nunca en Plan ni seed de 299 MXN/3 instalaciones/7 días.
No son decisiones técnicas pendientes de Free: emisión Pro, billing/sync y planes privados siguen OUT/LATER con gates propios; ninguna licencia/grant Pro se implementa o emite aquí.

## 8. Matriz de cierre Q → recomendación → impactos → prueba exigida

Cada subfila descompone literalmente las preguntas de §5–9 candidato; toda recomendación es P, sustentada por N donde se cita arriba. S=SQLite, G=PG, F=Free, J=paridad Java.
Los identificadores de prueba siguientes son **locales a este contrato y futuros**, no gates normativos creados ni pruebas ejecutadas.

| Pregunta                         | Recomendación y sección                            | Impactos S / G / F / J                                                                                    | Prueba mínima futura                                                                                                |
| -------------------------------- | -------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Q-PB.1 lista org/overrides       | Una ACTIVE, org-only (§3)                          | S guard TX; G lock org; F local; J catálogo org preservado                                                | PB1 dos activaciones concurrentes: una vigente; cross-org rechazado.                                                |
| Q-PB.2 moneda org                | Bloquear cambio con datos monetarios (§3)          | S/G validar currency; F sin FX; J conservar moneda histórica                                              | PB2 cambio rechazado sin efectos, histórico idéntico.                                                               |
| Q-PB.3 IDs/fecha/vigencia        | Manifest/cutover + sucesión inmutable (§2–3)       | S/G índice scoped; F sin CP; J no precio histórico fabricado                                              | PB3 retry/restore; cancelar futuro y reemplazo inmediato/contiguo con revisión mayor; conservar pasado.             |
| Q-PB.4 código/ausencia           | 64/120 y fallo sin precio (§2–3)                   | S/G misma forma/UK; F bloquea solo carrito afectado; J sin fallback                                       | PB4 longitudes límite, normalización duplicada, precio ausente.                                                     |
| Q-TP.1 jurisdicción/fuente       | Catálogo local explícito y Branch configurada (§4) | S/G campo/validación; F offline; J ausencia legada explícita                                              | TP1 jurisdicción ajena/futura/ausente, autorización.                                                                |
| Q-TP.2 regla/conjunto            | Perfil componente, N reglas/asignaciones (§4.1)    | S/G relación nueva/UK ajustadas; F máximo 8; J no impuestos sintéticos                                    | TP2 dos tributos; APPLY→STOP→APPLY, NULL rule solo STOP, FK slot cruzado rechazado; 0/exento/no objeto distintos.   |
| Q-TP.3 selección/vigencia        | Product→rule concreta por operación/slot (§2/4.1)  | S guard producto; G lock; F resolver local; J IDs nuevos separados                                        | TP3 STOP y batch NOT_SUBJECT único, fin implícito inválido; cancelar futuro sin frontera, quote obsoleto.           |
| Q-TP.4 ventas legadas            | Preservar ausencia y snapshot (§4.2)               | S/G FK nullable legado; F refund histórico; J importes intactos                                           | TP4 refund 5u/3¢ = 1,0,1,0,1; particiones/fracciones, fiscal neto+traslado−retención concilia sin exceder snapshot. |
| Q-TP.5 rounding/incompleta       | TAX_SIMPLE_1/POS_LINE_HE_1 explícitos (§4.2)       | S TEXT/G NUMERIC; F error previo a cobro; J sin redondeo de historia                                      | TP5 NET/GROSS multi-tasa/retención, empate/centavos, 9 componentes y cuotas rechazados.                             |
| Q-CP.1 scope/item                | Branch-wide, cálculo por item (§5)                 | S/G FK org/branch; F MWAC único; J sin FIFO ni historia nueva                                             | CP1 aislamiento item/branch y vectores del anexo de valoración §8.                                                  |
| Q-CP.2 numérico/residuo/sucesión | MWAC-DEC1 por anexo D06; lifecycle (§2/5)          | S texto checked; G escala validada antes de insert; F sin licencia; J escala 6×4 preservada               | CP2 vectores D06 del anexo; cancelación futura/reemplazo, competencia/retiro y contrato desconocido.                |
| Q-CP.3 receipt histórica         | Ausencia≠cero por anexo D03–D04 (§5)               | S/G mapping del anexo D03–D04; F sin costo confirmado falso; J receipt original preservada                | CP3 NULL/cero, cadena ambigua, costo PO no copiado.                                                                 |
| Q-CP.4 pending/ajuste/cutover    | Continuidad D05 y resolución por anexo (§5)        | S/G seguimiento del anexo; F salida física pending autorizada sin costo confirmado; J sin replay de stock | CP4 ajuste/retry/crash, saldo físico idéntico, pendiente no curado por compra.                                      |
| Q-EC.1 etiqueta histórica        | Identidad estable/etiqueta actual (§6)             | S/G sin snapshot nuevo; F reporte honesto; J módulo nuevo                                                 | EC1 renombre no reclasifica gasto; export rotula etiqueta actual.                                                   |
| Q-EC.2 reclasificación           | Solo DRAFT, luego compensar (§6)                   | S/G CAS/audit; F pago previo permitido; J no tocar historia                                               | EC2 POSTED inmutable, CASH retry uno/BANK cero, categoría inactiva.                                                 |
| Q-EC.3 inicialización            | Alta explícita del titular (§6)                    | S/G sin seed implícito; F no CP; J no convertir Product.category                                          | EC3 falta categoría falla, mismo code normalizado colisiona, nombre repetido válido.                                |
| Q-EC.4 scope                     | Org compartido, Branch del gasto opcional (§6)     | S/G FK compuesta con NULL branch; F una operativa; J sin categorías branch inventadas                     | EC4 cross-org rechazado con branch NULL y no NULL.                                                                  |
| Q-PL.1 global/privado/código     | Global; Enterprise privado LATER (§7)              | S no Plan obligatorio; G UK code global; F independiente; J sin origen 1:1                                | PL1 huérfano/versión duplicada falla; misma versión otro plan válida.                                               |
| Q-PL.2 nombre contractual        | Snapshot/términos/digest en versión (§7)           | S nada remoto requerido; G nuevos campos; F no afectado; J no import comercial ficticio                   | PL2 renombrar Plan no altera términos/grants/RBAC.                                                                  |
| Q-PL.3 desactivación             | Bloquea altas, no derechos emitidos (§7)           | S safe point; G guard contratación; F preserva core; J historia intacta                                   | PL3 inactive no contrata; downgrade/exceso conserva lectura/export.                                                 |
| Q-PL.4 publica/evidencia         | Preparador≠aprobador, digest/grants/tests (§7)     | S política Free local; G audit y permiso CP; F sin Pro; J sin suscripción falsa                           | PL4 no autorizado/keys inválidas/core=false denegados; checkout sin red/CP/License.                                 |

## 9. Condiciones de aprobación y validación documental

El cierre técnico propuesto es completo para D02; la aprobación real debe registrar opción elegida, alcance, versión, fecha, titular/revisores y referencias, sin tratar este texto como aceptación.
Paquete a reconciliar por sus editores autorizados: F1-10 padres + hijos/scopes/lifecycle + ficha de asignación + Branch fiscal + snapshots + PlanVersion; F1-09/08 ER/diccionario; F1-11 pruebas; F1-12 manifest/ausencias; F4-13/15/17/21 referencias afectadas.
No sumar cinco padres para declarar un total final: ProductTaxAssignment y cambios en hijos son complejidad adicional; seguimiento económico se define solo en el anexo D03–D06 y se tramita por CC21-04/05. D01 debe conservar 82 fichas comprobadas sin inventar la 83.
Ningún nuevo campo aquí es canonical ni DDL autorizado. Si se aprueba solo la alternativa fiscal mínima, registrar su alcance y rechazo explícito de operaciones multiimpuesto; no anunciar paridad funcional completa.
Antes de materializar: fuentes canónicas versionadas/aprobadas y ADR21 actualizado por titular; después pruebas unitarias + integración real S/G, escritura inválida directa, concurrencia, rollback/restore y gates DDL13/PAR/AS/IMP aplicables.
Validación de este entregable: cobertura de las 21 subpreguntas, referencias/huellas, límites N/P/R, Markdown/enlaces locales, whitespace y write set; vectores sintéticos de refund por particiones/desglose y cronología APPLY/STOP/cancelación, sin tests Java/Rust/SQL ni cambios de estado de gates.
Se mantienen bloqueados el import afectado, full completion 0b y cutover. La ejecución de pruebas futuras no se sustituye por esta matriz ni por la aprobación documental.
