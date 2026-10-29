# ADR-0021 / CC-0021 — paquete de aprobación y preparación de 0d

**PROPOSED — DIRECCIÓN APROBADA CON CONDICIONES · revisión PRE0D-1 · 2026-10-28.**
La dirección funcional D01..D07 fue aprobada por el titular el 2026-10-28 con
condiciones (§5). **No es aceptación del ADR ni del CC**: aprobador por decisión,
revisores, fecha de cierre y revisiones canónicas nuevas siguen **PENDIENTES**.
**Lectura ejecutiva: §1, §2 y §5.** Los anexos cierran recomendaciones técnicas;
no autorizan DDL ni cambian el freeze.

## 1. Decisiones que se someten a aprobación

| ID  | Recomendación final P                                                                                                                                                                         | Alternativa no recomendada y motivo                                                                 | Impacto/aceptación determinante                                                                                                                                                                                    |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D01 | Corregir inventario v1.0 a **82 fichas verificadas (59/9/14)**, sin crear entidad 83. Nuevas dependencias se cuentan separadamente al aprobar sus fichas.                                     | Inventar tabla para cuadrar 83; confundir objeto HTML vacío con entidad.                            | Corrección editorial, no DDL. No afirmar causa del error sin evidencia del autor.                                                                                                                                  |
| D02 | Padres PriceBook/TaxProfile/ExpenseCategory org-owned, CostingPolicy branch-wide aplicada por item, Plan global CP; lifecycle/selección según anexo D02.                                      | Padres solo ID, duplicar PascalCase/snake_case, impuestos implícitos o planes como RBAC.            | Aceptar **21 respuestas Q concretas**, relación fiscal explícita adicional y límites de configuración propuestos; no se fijan tarifas ni se activa Pro.                                                            |
| D03 | **B+D**: costo pactado, efectivo, valoración y procedencia separados; NULL≠0≠provisional.                                                                                                     | Obligar costo al pasado o inferir de catálogo/promedio/pactado.                                     | Preservar todos los hechos Java; estados nuevos describen evaluación AT_IMPORT, no supuesta ejecución Java.                                                                                                        |
| D04 | PO nueva pactado opcional; receipt nueva requiere documento o **atestación local autorizada** de costo/base/cantidad/unidad/moneda, con artefacto y digest disponibles offline.               | Factura/remoto obligatorio; o nueva receipt sin dato económico como regla ordinaria.                | Cero requiere evidencia explícita. Sin evidencia no POSTED; guardado de trabajo sin efectos permitido. No se aplica retrospectivamente al import.                                                                  |
| D05 | Permitir continuidad física sobre legado íntegro pending, con seguimiento atómico y **sin costo/margen confirmado**. STRICT permanece; negativo de venta exige policy y permiso propuestos.   | Bloquear todo checkout por desconocimiento histórico; inventar costo para continuar.                | Aprobar excepción stock+seguimiento cuando no puede existir asiento numérico. RESELLABLE afectado por déficit/pending se deniega sin efectos; devolución física separada no autoriza refund ni reingreso vendible. |
| D06 | **MWAC-DEC1**: Q(19,6), c(19,4), total(38,10), V/COGS(76,18); HE(V×s/Q,18), último consumo C=V. Promedio visible12 nunca operando. Cota/trace explícitos; historia ambigua no confirma valor. | Promedio decimal redondeado recursivo, FIFO positivo o racionales persistidos de complejidad mayor. | Mantiene promedio móvil; cuantización derivada es explícita/versionada, no pérdida de fuentes. Exceso de rango/cota rechaza, no degrada a pending.                                                                 |
| D07 | Adoptar mapping **47/47 compras + 12/12 auditoría** y FK de ledger, con ausencias tipadas. Conservar snapshots originales.                                                                    | Reconstruir con catálogos actuales, generar auditoría histórica o reenviar receipts para importar.  | Proponer `version` persistida distinta de expectedVersion, rechazo nullable legado, JTI solo forense y eliminar vínculo inverso redundante de movimiento. Nuevas reglas no se imponen al pasado.                   |

**Propuesta conjunta:** D01..D07, con anexos versionados y excepciones expresas.
Ninguna fila está ACCEPTED. No se declara IMP-01/DDL13 aprobado por aceptar el diseño.

## 2. Alcance de las respuestas y preguntas realmente abiertas

No quedan decisiones técnicas delegadas a un genérico «por definir» en las Q del
candidato: [D02 §8](rust-d02-approval-contract.md#8-matriz-de-cierre-q--recomendación--impactos--prueba-exigida)
responde sus 21 subpreguntas, con lifecycle, scopes, FK/índices, longitudes, moneda,
selección multiimpuesto, publicación, cancelación futura, STOP fiscal y refund acumulativo.
DEC1 fija precisión, redondeo, residuos, negativos, costo tardío, cota y causalidad.
D07 fija destinos y ausencias. Las elecciones sin mandato de la fuente se marcan P.

**Condiciones que la aprobación de dirección no cierra (las decide el titular, no el agente):**

1. ¿Acepta el subconjunto fiscal TAX_SIMPLE_1/POS_LINE_HE_1, límites propuestos y
   su revisión fiscal para el negocio? Si no cubre casos reales, ampliar contrato
   antes de habilitar productos, no aproximar tasas/reglas.
2. ¿Acepta atestación local de costos y quién tiene responsabilidad/permiso para
   aportarla, resolver valoración y autorizar una base económica futura? Esto no
   exige elegir nombres de personas ficticios para completar un fixture.
3. ¿Acepta continuidad física pending, margen no disponible, permiso adicional
   de negativos y limitación de retorno vendible afectado? Cambian operación nueva.
4. ¿Acepta DEC1 y su cota de 10⁻⁸ unidades monetarias, liquidación de deuda y
   contrapartida de residuo excepcional, sin reescribir conocimiento histórico?
5. ¿Autoriza estrategia Git B de §6 y las protecciones? Son aprobaciones separadas
   de dominio y no se infieren de aceptar ADR-0021.

**Datos a configurar/acreditar, no dudas de arquitectura:** moneda/jurisdicción,
tributos/tasas reales, categorías, actores habilitados, IDs de configuración,
instante de cutover futuro y evidencias de costo/orden. Los fixtures usarán datos
sintéticos explícitos. Tarifas/cuotas/grace Pro y planes privados siguen LATER.
La causa histórica del 83 no está probada; una errata de conteo puede aprobarse
sobre el inventario comprobado sin atribuirle una causa ficticia.

## 3. Contrato compuesto y precedencia de propuestas

1. Este resumen y [CC-0021](rust-adr0021-change-control.md) delimitan qué se pide.
2. [D02](rust-d02-approval-contract.md) concreta los diccionarios de padres e hijos;
   el candidato 0c sigue como antecedente. D02 no define otro algoritmo económico.
3. [D03–D06 / DEC1](rust-valuation-approval-contract.md) es la única recomendación
   económica final; contiene **16 vectores** y matriz permitir/denegar.
4. [D07](rust-d07-approval-contract.md) completa el mapping campo por campo de 0c
   con destinos/scope/audit y ausencias. Sus deltas están sujetos al mismo control.
5. F4–21 y documentos canónicos vigentes **prevalecen sobre todas estas propuestas**.
   No se copian exports confidenciales; huellas y referencias figuran en anexos.

| ID  | Dominio y compatibilidad Java                                                         | SQLite / PostgreSQL propuestos                                                                                           | Free Local / riesgos / aceptación                                                                                                       |
| --- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| D01 | Baseline físico comprobado, Java sin cambios                                          | Sin delta SQL por corregir cifra                                                                                         | No convierte 82 en meta del modelo futuro; conteo derivado después de cambios aprobados.                                                |
| D02 | Configuración nueva separada de hechos Java; sin impuesto/moneda histórica inventados | TEXT STRICT vs tipos nativos; claves compuestas, relación fiscal APPLY/STOP, cancelación futura y guards transaccionales | Config incompleta bloquea solo operación afectada, no necesita CP. 21Q y tests PB/TP/CP/EC/PL; revisar alcance fiscal.                  |
| D03 | Preservar NULL y ausencia propia de receipt; no pérdida por nueva obligación          | Nullable conjunto de costo/total y followup append-only tipado, nunca asiento cero                                       | Pending es falta de base, no estimación. Tests NULL/cero/positivo y no efecto parcial.                                                  |
| D04 | Diferencia de producto nueva, no requisito retroactivo                                | Evidencia+receipt+stock/valor o pending+PO+audit/dedup/outbox atómicos                                                   | Artefacto local verificado; atestación admitida P, no certificación fiscal; probar crash, replay, scope y red denegada.                 |
| D05 | Fuente física intacta; nueva continuidad explícita                                    | Estado inicial inmutable y proyección/revisiones, followup físico sin policy solo cuando no hay valoración               | No utilidad confirmada; permisos no son plan. Probar STRICT, pending/negativos, retorno rechazado y caja independiente.                 |
| D06 | Fuentes exactas; costeo Rust identificado como cálculo nuevo, no supuesto costeo Java | TEXT exacto / NUMERIC(76,18), enteros de secuencia y cota; intermediarios exactos acotados                               | Cota conservadora puede detener comando valorado; nunca reset automático. V01..16, concurrencia, límites, late cost y órdenes ambiguos. |
| D07 | 47+12 campos, IDs/textos/instantes exactos, sesiones Java no importadas               | FK movimiento→receipt line única evita ciclo; audit.session_id NULL legado, legacy_token_jti forense                     | No reconstrucción desde catálogo. Comparar digests, append-only, import repetido, fail-fast y restore completos.                        |

## 4. Cambios canónicos exactos propuestos (NO aplicados)

| Fuente/sección                                                                    | Deltas que deberá publicar su editor autorizado                                                                                                                                                                                                                                                             | Sin cambio                                                                                            |
| --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| F1–10 inventario/§2–3/6–7/10, §16.14–19/21–23/37–41/60–62, §17.2                  | D01 inventario 82 del baseline; D02 cinco fichas+asignación fiscal/slots/scopes/lifecycle/Branch fiscal/PlanVersion; D03–06 NULL, followup, escala/trace/cota/causalidad/proyecciones; D07 snapshots/actores/tiempos, version, rechazo nullable, FK inversa redundante retirada y correlación audit forense | No tabla 83 por aritmética; raíces/historias SQLx independientes; Money final minor.                  |
| F1–11 §4 LOC-05/10/11/13/14/15/17, §6 T-005/010/011/019, §7 DDL-01/03/04/05/12/13 | Ampliar conformidad por scopes, temporalidad, STOP, pending/evidencia, DEC1/cotas/trace, causalidad, snapshots, crash/restore y replay                                                                                                                                                                      | No reducir thresholds, IDs ni gates; cero tolerancia a pérdida/redondeo silencioso.                   |
| F1–12 §2 compras/audit/ledger/stock/ventas y configuración sin origen; §4/6       | Mapping 47+12; manifiesto/config desde corte, escala/NULL/ausencias, orden derivado≠causal original, money no representable falla; acciones explícitas por tabla, no receipts reejecutadas                                                                                                                  | 37/37 origen, UUID estables, sesiones INVALIDATE, PAR/AS obligatorios.                                |
| F4–13 §3–10/13                                                                    | Añadir DEC1, cuantización/cota y separación de pendiente/provisional; input exacto vs costo derivado cuantizado; costo tardío/deuda/residuo; referir contrato fiscal propuesto de D02 sin vender cumplimiento SAT                                                                                           | Promedio móvil Branch/Item, sin float, ledger separado append-only.                                   |
| F4–15 §2, §4 Sale/PO/Receipt/StockCount y postventa, §5–6                         | Pactado opcional, evidencia receipt nueva, transición física+followup pending atómica, permiso negativo, retorno afectado rechazado, versiones/CAS/slots fiscales y refunds acumulativos                                                                                                                    | Terminales históricos y límites de ownership; no borrado compensatorio silencioso.                    |
| F4–21 §1 Inventario/Compras/Ventas/Postventa, §2 Money/Costeo/Writer, §5–7        | Referenciar revisión aprobada completa y excepciones acotadas; actualizar referencia editorial al inventario v1.0 separada del conteo futuro                                                                                                                                                                | Free offline sin CP/Stripe; single-writer; IMP-01..09 y prohibición de import real/cutover sin gates. |

También sincronizar F1–08/09 ER/diccionario, F4–17 para el subconjunto fiscal y
OpenAPI/ApiError cuando exista implementación visible, sin reescribir ADR aceptados
anteriores ni el DDL Java. No cambiar canónicos simplemente al crear este documento.

## 5. Propuesta de aprobación para el titular

**Registro de la autorización condicionada, sin aceptación prellenada:**

| Registro                                                              | Estado                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Dirección funcional D01..D07                                          | **APROBADA CON CONDICIONES** por el titular, 2026-10-28.                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Alcance mínimo ratificado                                             | D01 reconocer 82 fichas sin inventar entidad; D02 ownership/catálogos/scopes propuestos con las 21 Q por cerrar antes del DDL afectado; D03 separar pactado/efectivo/valoración/procedencia con NULL≠0; D04 evidencia documental o atestación local autorizada en recepciones nuevas; D05 continuidad física sobre pending sin inventar costo/margen; D06 MWAC-DEC1 como propuesta numérica a validar por pruebas; D07 preservar 47 campos de compras y auditoría, distinguiendo existencias y ausencias. |
| Condiciones aún abiertas                                              | Subconjunto fiscal y revisión aplicable; responsables de la atestación local; permiso de negativos y límites de retorno vendible; aceptación formal de DEC1/cota/regularizaciones; estrategia Git y protecciones.                                                                                                                                                                                                                                                                                         |
| Estado del ADR-0021 / CC-0021                                         | **PROPOSED**. La dirección aprobada no los convierte en ACCEPTED.                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Revisores independientes de dominio/datos/seguridad                   | **PENDIENTES**.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Fecha de cierre, SHA del paquete y versiones/huellas canónicas nuevas | **PENDIENTES**.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Estrategia Git B                                                      | **PREPARACIÓN AUTORIZADA**; ejecución de la reversión sujeta a revisión final y autorización separada.                                                                                                                                                                                                                                                                                                                                                                                                    |
| Protecciones de rama                                                  | **PROPUESTAS**, no autorizadas para aplicar.                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

La aprobación de dirección no acepta campos implícitos ni habilita DDL. Una
aprobación parcial se registra por ID y bloquea sus dependencias no aceptadas. Tras
cerrar condiciones, actualización canónica y verificación cruzada, ADR-0021 podrá
pasar a ACCEPTED con fecha/evidencia. Hasta entonces **PROPOSED** e import afectado
bloqueado.

## 6. Reconciliación Git y recuperación reversible de integración

Base nueva confirmada tras fetch: `origin/dev@329c0984722e8339f3e3b29497960d426ac2c43c`;
HEAD inicial igual, worktree limpio. Una consulta API posterior en esta sesión
confirmó que el run más reciente de `dev` sigue siendo el 40775256216, `failure`,
sobre ese mismo SHA. `43d9909` y `329c098` son descendientes ya
publicados de `6a15ee6`, sin PR asociado observado; no se duplicaron. Reflog registra
`update by push` el 2026-10-28 15:50:34 -0600. Eso no atribuye responsables ni permiso.
El informe 0c describía su estado local en otro momento: su afirmación global de
«sin push/CI nuevos» no es el estado remoto actual.

[Run 40775256216](https://github.com/HiQNet-Dev/point-of-sale/actions/runs/40775256216),
SHA `329c098`, comenzó in_progress y después terminó **failure**: Java/React/Compose
success; único step Rust fallido `Require complete normative migration evidence
(blocks merge)`; CI Gate falla `Verify required jobs`; E2E skipped. Las regresiones
del verificador 0c corrieron; esto no valida el nuevo cambio de 17 tests.
No hay PR abierto observado. `main` remoto sigue `2f0d6983325523a17a8a22ba8aa82e49b4994948`.

APIs de protección dev y main: 404 explícito **Branch not protected**; reglas activas
aplicables de ambas: `[]`. No se editó configuración, ref dev/main ni historial.
CI puede fallar después de un push; sin enforcement no impide que ocurra.

| Estrategia                                                              | Ventaja                                                                      | Riesgo / procedimiento propuesto                                                                                                                                                                                                                                                                                           |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A — conservar Rust experimental en dev hasta cierre                     | Sin revert/reapply; Java/React correctos                                     | Aceptar explícitamente integración roja temporal, congelar promoción a main y nuevos esquemas, proteger ramas y completar 29 casos en features. Un PR docs-only verde no sanea el árbol incompleto.                                                                                                                        |
| **B — PR de reversión coherente, recomendado para recuperar dev verde** | Dev vuelve al alcance Java estable mientras Rust conserva historia y feature | Tras autorización, preservar ref de los ocho commits publicados en rama no productiva; PR desde dev con reversión inversa de **los ocho**, no solo el gate, sin borrar historia. Revisar conflictos y ejecutar Java/React/Compose/E2E. Reintroducir todo Rust mediante PR con gates completos, no cherry-picks duplicados. |

**Recomendación actual: B**, si la exigencia es integración verde estable; supera
la recomendación A condicional de 0c a la luz de otra publicación con gate rojo.
No se ejecuta hoy. La reversión sería del experimento completo y de su integración,
no una excepción para aprobar el mismo esquema quitando `--require-complete`.
El gate permanece íntegro en la línea Rust y deberá volver con ella; si el titular
exige que también permanezca en el árbol dev, B no recupera verde y debe elegir A.
Reversibilidad: conservar commits y volver a introducir el cambio revisado/ajustado
por PR; nunca reset/force-push/rebase publicado. Cambios de terceros posteriores
requieren evaluación antes de revertir; no ejecutar una receta ciega sobre SHAs.

### Protección propuesta, no aplicada

- Rulesets activos separados dev/main, PR obligatorio, ≥1 revisión humana
  independiente, invalidar aprobaciones ante nuevo diff y aprobación de último
  push por persona distinta. Sin bypass rutinario de admins/apps.
- `CI Gate` requerido, emisor observado **GitHub Actions app 15368**, rama al día,
  conversaciones resueltas; bloquear force-push y borrado. No usar jobs condicionales
  como único requisito. Main añade política release/hotfix y auditoría, no push directo.
- Excepción de emergencia: autorización titular+revisor, alcance/SHA/motivo/plazo,
  procedimiento de rollback, registro auditable, PR aun en emergencia, restaurar
  enforcement y post-review. No lista permanente de bypass para el agente.
- Verificar aplicación con GET protection/rules y PR controlado rojo/sin review;
  no probar mediante push real no autorizado. No activar merge queue sin adaptar
  workflow a merge_group. No imponer historia lineal a main que contradiga releases.

Fuentes oficiales de comportamiento:
[Protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches),
[Rules REST](https://docs.github.com/en/rest/repos/rules#get-rules-for-a-branch).

## 7. Preparación ejecutable de 0d y resultados reales

| Lote                 | Puede comenzar ahora                                                                                          | Depende de aprobación/cambio controlado                                          | Salida verificable                                                                  |
| -------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| 0d-A infraestructura | Verificador/fixtures sintéticos/checksum/procedencia, tests de fallos/restore/aislamiento sin inventar modelo | No requiere adoptar D01..07                                                      | Pruebas técnicas y evidencias etiquetadas, sin PASS normativo nuevo.                |
| 0d-B baseline físico | Preparar inventario/crosswalk/diffs y casos de prueba; sin aplicar candidatos                                 | Aceptación ADR/CC, fuentes canónicas coherentes, revisión de diccionarios/anexos | Manifest versionado, conteo por contexto y cero FK sin ficha.                       |
| 0d-C DDL completo    | No materializar aún padres/contratos controvertidos                                                           | 0d-B y gates correspondientes                                                    | Migraciones aditivas Local/Central/Control, clean DDL13-01..03/11 reales.           |
| 0d-D evolución       | Mejorar harness de fixtures ahora                                                                             | Esquemas aprobados para aprobar normativos                                       | N-1→N, checksum/failure/restore/rerun DDL13-04..09 y offline red denegada DDL13-10. |
| 0d-E import/paridad  | Preparar casos, exporter y validación fiel; sin importer que aplique supuestos                                | Modelo completo+contrato aprobado, no autorización productiva                    | Import sintético versionado/idempotente, PAR restantes y AS completos.              |

Cambio seguro ejecutado aquí: el verificador ya no acepta 30 registros si falta un
ID sustituido por otro duplicado. Exige IDs exactos/únicos, estados válidos y
contadores enteros derivados; se conservan PAR-08 único PASS y exit2 normativo.
Tests pasaron de 8 a **17**; regresión RED reprodujo defecto antes del arreglo.
CI existente ya ejecuta esa suite; no se cambió YAML ni significado de PASS.
En la misma línea se añadió `flyway_worktree` al baseline: detecta migraciones
Java modificadas o untracked en el checkout para no presentarlas como fuente
íntegra. Es informativo y no altera gates. También se creó el
[inventario Java AS-IS](rust-java-module-inventory.md) con la matriz de módulos,
endpoints, tablas, pruebas y deudas observadas antes de portar.

Comandos reproducibles desde backend-rust (salida debe ser nueva):

```sh
python3 -B tools/migration-lab/scripts/test_verify_evidence.py
cargo fmt --all -- --check
cargo run --locked -p migration-lab -- --output target/approval-pre0d-gate-1 --postgres-bin target/pg17-build/install/bin --java-source --require-complete
# Hoy el comando anterior devuelve 2: NO interceptarlo para aprobar CI.
python3 tools/migration-lab/scripts/verify_evidence.py --evidence target/approval-pre0d-gate-1 --expect-exit 2
cargo audit --file Cargo.lock --json
```

Resultados nuevos de la revisión 0d: **17 tests verificador OK**, **21 tests CI OK**,
fmt OK, Clippy `-D warnings` OK, **60 tests Rust listados** y suite workspace verde,
RustSec **0 vulnerabilidades/warnings vacíos**. La ejecución
`target/approval-pre0d-gate-2` (SQLite, sin PostgreSQL/Java externo) dio
**19/19 técnicos**, `exit=2` y **0/30 normativos**. La ejecución previa
`gate-1` con PostgreSQL 17.11 + source + auth dio **41/41 técnicos**, `exit=2` y
dos instalaciones Flyway V1–V25 reales: 37/37 tablas y digest funcional igual.
El verificador de `gate-2` confirma que solo PAR-08 está aprobado a nivel de
exporter. No se alteró código Java/React, lock ni migrations. No se reejecutaron
suites locales Java/React/E2E; se reporta su resultado remoto base por separado.
Las revisiones numéricas de anexos son oráculos propuestos, no implementación ni
nuevas pruebas normativas.

Evidencia: `backend-rust/target/approval-pre0d-gate-2/`, con JSON/JUnit/manifest/
logs/summary. `flyway_worktree` quedó `clean: true`, `paths: []`; el inventario
observado sigue en **25 Flyway / 37 tablas**. HEAD de procedencia `329c098`, dirty
por cambios en elaboración; no se presenta como CI remota nueva ni como un SHA
final limpio. SHA-256 de parity-report.json de `gate-1`:
`c909f1c69ec90f927c84414ebeb4e015d91baf3b679732f7760165f621373a44`.

El lote de estrategia B conserva `inverse.patch`
`fa89970e…f505295` y `manifest.json` `9cfd17c8…c6cd8da`. Su verificación de
preservación generada antes de estos documentos ya no coincide: es esperado,
porque esta revisión añadió documentación autorizada. No invalida el parche ni el
árbol virtual ya probado, y no se regeneró ni sobrescribió la evidencia.

| Matriz normativa        | Estados conservados                                                       |
| ----------------------- | ------------------------------------------------------------------------- |
| DDL13-01..09 y DDL13-11 | **BLOCKED**, diez casos: subconjuntos técnicos no son esquemas completos. |
| DDL13-10                | **NOT_RUN**, no prueba normativa offline completa con red denegada.       |
| PAR-08                  | **PASSED**, solo determinismo source exporter.                            |
| PAR-01..07 y PAR-09     | **BLOCKED**, ocho casos: falta import/comparación target.                 |
| AS-01..10               | **BLOCKED**, diez casos: no conformidad source↔target completa.           |

Total **1 PASSED + 28 BLOCKED + 1 NOT_RUN**. Tabla individual conservada en
[informe 0c §5](rust-increment-0c-report.md#5-los-30-escenarios-normativos), salida
machine-readable nueva indicada arriba. Aceptar este paquete no modifica esos
estados: solo evidencia real de cada caso permite completar `--require-complete`.

**Sin cutover, datos reales, Pro/Stripe ni retiro de Java.** Próximo paso exacto:
cerrar las condiciones de §5 y decidir el gobierno Git. Mientras tanto solo puede
avanzar 0d-A (infraestructura, procedencia, fixtures) y la preparación documental
de 0d-B/E; no el DDL contradictorio.
