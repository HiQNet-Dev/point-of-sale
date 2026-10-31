# CC-0021 — solicitud de cambio del físico y paridad Free Local v1

**Estado: PROPUESTO en su conjunto. `CC21-03` aceptada el 2026-10-30; el resto
sigue pendiente. Fecha de apertura: 2026-10-28.**
La dirección funcional D01..D07 fue **aprobada con condiciones** por el titular el
2026-10-28; las condiciones y el registro están en el [paquete PRE0D-1](rust-0d-approval-package.md) §5.
Relacionada con [ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md).
No edita documentos canónicos de Drive ni autoriza DDL, import o producción.
Revisores, aceptación formal del conjunto y versiones nuevas: **PENDIENTES**.

## 0. Registro de aprobación parcial

| Solicitud | Estado | Fecha | Alcance aceptado |
| --- | --- | --- | --- |
| `CC21-01` inventario | Pendiente | — | — |
| `CC21-02` padres incompletos | Pendiente | — | — |
| `CC21-03` costo pactado | **Aceptada** | 2026-10-30 | `unit_cost TEXT NULL`; `NULL` = desconocido, `0.0000` = cero conocido; total derivado `NULL` si falta algún costo; reportes distinguen «sin costo»; sin inferir costo efectivo de recepción |
| `CC21-04` costo efectivo/valoración | Pendiente | — | — |
| `CC21-05` snapshots | Pendiente | — | — |

Fuente de la aceptación: `00B · Control maestro de revisiones y aprobaciones —
HiQNet POS` (2026-10-30). La aceptación de `CC21-03` **no** aprueba `ADR-0021`
completo ni las demás solicitudes.

## Versión de aprobación PRE0D-1

El [paquete ejecutivo](rust-0d-approval-package.md) fija la recomendación por
D01..D07, alternativas, impacto y criterios; [D02](rust-d02-approval-contract.md)
responde las 21 Q técnicas, [D03..D06](rust-valuation-approval-contract.md) fija
MWAC-DEC1 y la operación pending, [D07](rust-d07-approval-contract.md) completa
mapping/ausencias. Son anexos de esta solicitud, **no aprobaciones**. Sus deltas
finales concretan las filas históricas siguientes; no quedan autorizados por
formular una respuesta técnica. Prevalece F4–21 vigente hasta aceptación y
actualización canónica. Las preguntas abiertas se reducen a aceptación de negocio,
responsables/configuración real y gobierno Git, no a decisiones técnicas implícitas.

Alcance adicional explícito CC21-02/04/05: asignación fiscal versionada APPLY/STOP,
followup económico append-only, evidencia local, continuidad física pending sin
valor, escala/cota DEC1, refund acumulativo, cancelación futura, retiro de FK
inversa redundante, rechazo desconocido nullable y JTI forense sin sesión.
No se solicitan siete aceptaciones por arrastre ni una autorización productiva.
Los cambios exactos por F1–10/11/12 y F4–13/15/21 están en §4 del paquete.

## 1. Motivo y separación de decisiones

El laboratorio encontró hechos Java representables en el origen pero incompatibles
con el destino NN de compras. Además faltan cinco fichas padre y destinos de
snapshots. No es una propuesta de reemplazar promedio móvil, Money exacto,
single-writer, identidad o append-only. Es necesario cerrar su representación
sin fabricar historia. Una corrección editorial del conteo se tramita aparte de
cambios de nulabilidad/valoración y no autoriza los segundos.

## 2. Solicitudes y documentos afectados

| Solicitud                      | Baseline actual                               | Cambio recomendado para aprobación                                                                                                                                                       | Fuentes/secciones a actualizar por su titular                                                                                                                                                                                 |
| ------------------------------ | --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CC21-01 — inventario           | 83 declaradas; 82 fichas verificadas          | Confirmar errata o identificar ficha ausente con nombre/contrato. No completar por resta. Mantener conteo derivado de fichas.                                                            | F1–10 inventario, F4–21 §6 y anexos que repitan cifra. Si solo editorial, registrar corrección como tal.                                                                                                                      |
| CC21-02 — padres incompletos   | FK a cinco dependencias sin ficha             | Revisar diccionarios candidatos, ownership y todas preguntas Q; fijar nombres físicos, campos, claves e índices. No aceptar solo nombres.                                                | F1–10 §16.18–19,37–38,60 y nuevas fichas; F1–11 FK/scopes; F1–12 mapping de entidades sin origen.                                                                                                                             |
| CC21-03 — costo pactado        | §16.40 costo y total NN; Java/F1–12 nullable  | Nullable conjunto, semántica desconocido/cero/positivo, producto exacto/rango y total derivado explícito; preservar estados Java.                                                        | F1–10 §16.40; F1–12 §2/4; F1–11 vectores; F4–15 aclaración de emisión sin costo pactado obligatorio.                                                                                                                          |
| CC21-04 — costo efectivo/valor | §16.41 costo NN; receipt Java sin dato propio | Frontera histórica y estado/procedencia de evidencia/valoración; ningún asiento falso; recepción nueva con base documentada. Regularización append-only y propagación a cadena afectada. | F1–10 §16.14–18/40–41 y proyecciones relacionadas; F1–12 compras/ledger/baseline económico; F1–11 tests; F4–13 reglas de pendiente y regularización; F4–15 transacción receipt. F4–21 §1/2/6 referencia baseline actualizado. |
| CC21-05 — snapshots            | Datos fuente sin destino explícito suficiente | Aprobar mapping de 47 campos y ledger, incluidos actor/tiempo, moneda, unidad, código/nombre y alcance de idempotency key. Explicitar datos inexistentes.                                | F1–10 §16.15–17/40–41; F1–12 §2; F1–11 constraints/immutability y F4–15 scope.                                                                                                                                                |

[Fichas propuestas de dependencias](rust-missing-physical-proposal.md) y
[mapping exhaustivo](rust-purchase-parity-contract.md) son anexos de trabajo,
no cambios canónicos. Si las revisiones requieren entidades adicionales (p. ej.
selección de perfil tributario o seguimiento económico), cada una necesita ficha
y aprobación propia. No se fija hoy un nuevo conteo total del modelo.

## 3. Decisiones explícitas que necesita tomar el titular

| Decisión                   | Recomendación del agente, NO ADOPTADA                                                                                                | Registro exigido                                                                                                                                                                   |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D01 Inventario             | Reconocer 82 fichas comprobadas sin entidad 83 identificada; confirmar causa editorial con autor                                     | Errata confirmada o nueva ficha; versión/fecha.                                                                                                                                    |
| D02 Catálogos              | PriceBook/TaxProfile/ExpenseCategory organization-owned; CostingPolicy branch-wide versionada aplicada por item; Plan global Control | Aceptar/ajustar las 21 respuestas concretas de D02; sus lifecycle, límites y selección tributaria son P hasta aprobación, antes de DDL.                                            |
| D03 Historia económica     | B+D de ADR: NULL y evidencia faltante explícitos, no valor estimado sin fuente                                                       | Nulabilidad, enums, procedencia, campos/relaciones y reglas de import.                                                                                                             |
| D04 Operación nueva        | PO pactado opcional; receipt nueva requiere base económica documentada                                                               | Fuente admisible del costo, tratamiento ante dato faltante y aceptación de diferencia Java/Rust.                                                                                   |
| D05 Stock legado sin valor | No promedio/margen confirmado sobre cadena incompleta; regularización autorizada como hechos nuevos                                  | Aceptar/ajustar matriz D05: permitir físico pending autorizado, negar costo/margen confirmado sin base, STRICT íntegro y retorno vendible afectado rechazado; sin red obligatoria. |
| D06 Cálculo                | Contrato exacto versionado, sin truncamiento; política Q=0, negativos/residuo y costo tardío                                         | Aceptar/ajustar MWAC-DEC1, escalas/rangos/HE/cota y 16 vectores. Secuencia causal nueva; historia ambigua permanece pending, sin elegir fecha/UUID como causalidad.                |
| D07 Snapshots ausentes     | Conservar originales; ausencia tipada/procedencia, nunca reconstrucción silenciosa                                                   | Rechazos/motivos/facturas/impuestos no registrados: convención y límites de afirmación.                                                                                            |

Una aprobación parcial debe indicar sus límites; no habilita materializar el
resto. La aceptación del ADR no equivale a aprobar AUTHG-02, IMP-01 ni paridad.
No se propone modificar los parámetros de Argon2 del laboratorio para producto.

## 4. Procedimiento de control y aceptación

1. Titular y revisores de dominio/datos/seguridad seleccionan alternativas y
   completan decisiones anteriores. Registrar aprobación explícita y referencia
   de revisión; no atribuirla a una instrucción genérica de continuar.
2. Editores autorizados actualizan fuentes canónicas afectadas conservando IDs
   F1/F4, gobierno F3/F7, historial y enlaces. No sobrescribir retrospectivamente
   la aprobación v1.0; indicar nuevo baseline y qué cambió.
3. Registrar versión/revisión, fecha, autor/aprobador, URLs y SHA-256 de exports
   revisados. Un hash de URL mutable no es por sí solo aprobación.
4. Revisar consistencia entre fichas, ER, diccionario, constraints, mapping 37/37,
   F4–13/15 y freeze; conteo derivado por contexto y cero FK sin padre definido.
5. Solo entonces cambiar ADR-0021 a ACCEPTED con fecha/alcance verificables y
   congelar el baseline de implementación. Hoy sigue PROPOSED.
6. Implementar migraciones **aditivas** sobre raíces independientes, importer
   sintético versionado y pruebas. No editar `0001` publicado ni V1–V25.
7. Adjuntar DDL13/PAR/AS, AUTH, offline, restores y CI del SHA real; sin BLOCKER
   y con revisión independiente antes de merge/release correspondientes.

### Condiciones de rechazo

NULL→0; costo inferido de catálogo/promedio; pérdida de campos/snapshots; UUID
regenerados; filas/tablas omitidas sin regla; falta de owner/scope; JSON sustituyendo
FK; UPDATE del ledger; Pro/control remoto necesario para Free; gate debilitado;
no-equivalencia del importe/cantidad; aprobación normativa basada en fixture técnico.

## 5. Rollback y límites

Esta solicitud es documental: ningún cambio de datos que revertir. Si se rechaza,
se registra la razón y el ADR permanece propuesto/sustituido por decisión revisada;
se conserva el export fiel y el import bloqueado. Después de aceptación, cada
upgrade exige backup/digests y restore exacto antes de operaciones. No deshacer
historia con `reset`, force-push ni down destructivo; usar PR/forward-fix o restore
ensayado de bases sintéticas. No hay autorización de cutover, datos productivos,
usuarios reales, sync Pro, billing ni retirada de Java.
