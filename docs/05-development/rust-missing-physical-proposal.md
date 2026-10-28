# Incremento 0c — dependencias físicas faltantes

**Estado: PROPUESTO NO APROBADO.** Investigación: 2026-10-28.
Rama inspeccionada: `feat/backend/rust-physical-resolution`.
Write set exclusivo: este documento. Sin DDL, migraciones, cambios de código,
contrato HTTP, fuentes Drive ni estados de aprobación.

## 1. Objeto, límites y lectura de autoridad

Proponer un diccionario revisable para las cinco dependencias de
[ADR-0021](../01-architecture/adr/0021-frozen-physical-parity-conflicts.md),
contrastando la [auditoría física](rust-physical-audit.md) con exports actuales y
sus documentos de negocio. No sustituye el físico canónico F1-10 ni resuelve el
ADR: **no hay aprobador, aceptación ni autorización de materialización nuevos**.

Etiquetas usadas:

- **N — normativo del baseline consultado:** requisito explícito de F1-10,
  F1-12 o de decisiones ratificadas por F4-21, con sección identificada. No
  significa norma legal ni implementación verificada.
- **C — candidato documentado:** significado/campo del ER, diccionario, XLSX o
  documento funcional propuesto. Su existencia es evidencia; su obligatoriedad
  física no está aprobada por aparecer allí.
- **P — elección de esta propuesta:** campo, tipo concreto, default, constraint,
  índice, lifecycle o mecanismo de resolución sugerido aquí. Requiere revisión.
- **Q — pregunta al titular:** decisión pendiente; no se rellena por intuición.

Una fila `C + P` conserva un campo lógico localizado, pero propone aquí su
traducción física. **Todos los diccionarios de §5–9 son propuestas completas**,
no fragmentos del freeze ni nuevas fichas canónicas.

F4-21 §6–7 ratifica Free Local y el baseline técnico de F4-13..19. Por ello los
párrafos históricos «PROPOSED»/«todos abiertos» no anulan decisiones ratificadas;
tampoco la ratificación convierte cada campo de un anexo o cada hipótesis Pro en
aprobado. Un conflicto material se registra, no se decide por fecha o por mayoría
de documentos. F5-12 y F5-13 siguen siendo propuestas comerciales.

## 2. Investigación y fuentes accesibles

### 2.1 Registro de consulta y secciones

Se accedió sin credenciales adicionales a los exports siguientes. Se procesaron
en memoria; no se incorporaron copias del expediente al checkout. Solo se
registran referencias y decisiones derivadas mínimas, no extractos extensos.
Las URLs son **mutables**: el hash identifica los bytes consultados, no una
revisión inmutable de Drive ni una firma de aprobación.

| ID / export consultado                                                                                                                                                                                                               | Secciones utilizadas y alcance                                                                                                                    |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| [F1-10 TXT](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/export?format=txt)                                                                                                                       | §1–3, §6, §9–10, §15–17; especialmente §16.18–19, §16.37–41, §16.60–62. Dependencias, tipos y fronteras.                                          |
| [F1-10 HTML](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/export?format=html)                                                                                                                     | Estructura de tablas y bloque vacío entre `cash_register` y `cash_shift`; no revisión visual de diagramas.                                        |
| [F1-12 TXT](https://docs.google.com/document/d/16hyT4XALY3DrAc50RUTx-fESzMI0c5612zTKphHRieM/export?format=txt)                                                                                                                       | §2, §4, §6 y ratificaciones: mapping 37/37, UUID, exactitud, snapshots y costo nullable.                                                          |
| [F4-13 TXT](https://docs.google.com/document/d/1vL-yFrwenxeqbAjuWY9m5zOMhlMGADfVTDUeWaOa4OM/export?format=txt)                                                                                                                       | §3–10, §13 y ratificaciones: Money/DecimalExact, tax snapshots, promedio móvil, separación cantidad/valor y provisionalidad.                      |
| [F4-21 TXT](https://docs.google.com/document/d/1tIHbbRw5GhKOFkL4ff0Zoc-SmMc-q7J2WwyYXFjqiUE/export?format=txt)                                                                                                                       | §1–7: alcance Free, autoridad, gates y control de cambios.                                                                                        |
| [F1-08 TXT](https://docs.google.com/document/d/1T4yX0nRPN4dGKJKsyTbwemkH3EZrTzOT1cUbF6XDEJU/export?format=txt)                                                                                                                       | §6–9, §16–17: PriceBook/TaxProfile organization-owned, CostingPolicy Branch/Item; ER candidato.                                                   |
| [F1-09 TXT](https://docs.google.com/document/d/1aPZZcWmf3GvoNFfsLnpESg_Rym9V49-1DcTMwoxsvnw/export?format=txt)                                                                                                                       | §1–4, §6–11, §13–16: diccionario lógico, scopes, inventario y enlace al XLSX integral.                                                            |
| [XLSX integral enlazado por F1-09](https://drive.google.com/uc?export=download&id=14WObt9M5AIU-tCqUUuQMxh-X2dqZzKr3)                                                                                                                 | Hojas `Entidades`, `Campos`, `Decisiones P0`; inspección focalizada de las cinco dependencias, no auditoría de los 442 campos.                    |
| [Matriz F1-10 XLSX](https://docs.google.com/spreadsheets/d/1ruR5jU38WZFwwv_tzQay2Y28CqLYOfpbYSU7m4hyHD4/export?format=xlsx)                                                                                                          | Hojas disponibles: `Resumen`, `Entidades`, `Controles`, `Bloqueos P0`. Filas de las cinco dependencias en `Entidades`; no fichas campo por campo. |
| [F4-15 TXT](https://docs.google.com/document/d/12VYpuy-IznZufJpYiGqn-4BBi3JqeI0al3XCE5Gx7tU/export?format=txt)                                                                                                                       | §2, §4–6 y ratificaciones: claves/FK compuestas, lifecycle y Expense con branch opcional.                                                         |
| [F4-16 TXT](https://docs.google.com/document/d/1zI_TjkKOG7iBTT1sYe5223yZxBsVfi0B4CSZE6u9efg/export?format=txt)                                                                                                                       | §2–6, §8 y ratificaciones: catálogo comercial versionado, Free sin licencia obligatoria, downgrade no destructivo.                                |
| [F4-17 TXT](https://docs.google.com/document/d/1eUt4MmWDEzpCVuj6LmUBTD4zEvveH9lOMRbaBcr2hsU/export?format=txt)                                                                                                                       | §1, §3–4, §8–9 y ratificaciones: rounding explícito, snapshots y CFDI/PAC LATER. No se revalidó legislación SAT.                                  |
| [F5-09 TXT](https://docs.google.com/document/d/1_Fo4JkP546ucsJA6wiX6u8OK3lHSfScVQ-AoOKu4Ovo/export?format=txt)                                                                                                                       | §2, §5–6, §9–13: categoría de gasto operativo, devengo/pago y separación de compras/stock/caja.                                                   |
| [F5-10 TXT](https://docs.google.com/document/d/1AHq2wAbf4vY2U0oMW9bqw8F_Wdu-zugqkbYhHQDqs80/export?format=txt)                                                                                                                       | §3, §6–10, §13: precio base por organización, perfil tributario, método de costo y cardinalidades candidatas.                                     |
| [F5-12 TXT](https://docs.google.com/document/d/1QYer540xq2861EU5peA5aePLvbk-9dIRXemcI6Z71Zo/export?format=txt)                                                                                                                       | §2–5, §7, §11, §15–16: Plan es identidad comercial; PlanVersion contiene condiciones/capacidades. Control Plane propuesto, no implementado.       |
| [F5-13 TXT](https://docs.google.com/document/d/12wzZDsw-m8t0Fn8NOvniFQhvx-tAtO-4kxMnP4GH4uQ/export?format=txt)                                                                                                                       | §1, §3–7, §9–12: ediciones y límites; precio, cuotas Pro y grace no aprobados.                                                                    |
| [F1-11 TXT](https://docs.google.com/document/d/1PCgbE95Qt3GjIfy78rPt-tCMjQ2Ed2GwBzEcTfa4HMU/export?format=txt) / [F1-13 TXT](https://docs.google.com/document/d/1ZtDnxOeVjeBJD20gLI1GxA4cTY-bevRoL2fKUMqZBPk/export?format=txt)      | Revisión focalizada de F1-11 §4–7 y F1-13 §4–8: enforcement y gates DDL13/PAR/AS; no resultados de ejecución.                                     |
| [Índice maestro TXT](https://docs.google.com/document/d/1DfIuSJCjXE1eBurbWXzeEewHRj7ceBOMCSaHsGLpJ0U/export?format=txt) / [HTML](https://docs.google.com/document/d/1DfIuSJCjXE1eBurbWXzeEewHRj7ceBOMCSaHsGLpJ0U/export?format=html) | §1–5 y enlaces reales del HTML: localización de F4-16/F5-12. Cierre editorial no equivale a aceptación comercial.                                 |

También se leyó F1-07 §6, §14 como antecedente lógico enlazado por F1-09:
[export TXT](https://docs.google.com/document/d/1DAXibElGATp6FnQN5Zsa0H7ClBEeMqA4H29AjNxpOQ0/export?format=txt).
No se usa para imponer campos físicos adicionales.

**Limitaciones:** no se accedió al historial de revisiones/comentarios privados
ni a una aprobación nueva del titular. No se inspeccionaron FigJam/PNG ni se
certificó su equivalencia visual. El CSV sin `gid` de las matrices F1-09/F1-10
solo devuelve `Resumen` (seis filas incluido encabezado): no prueba haber leído
las entidades. Para esta investigación se abrió el XLSX completo de F1-10 y el
XLSX integral enlazado por F1-09. No se presupone que sus datos estén
sincronizados con las ratificaciones posteriores. No se reejecutó Java ni se
verificó runtime de SQLx, SQLite o PostgreSQL.

### 2.2 Huellas de evidencia

SHA-256 sobre bytes recibidos, sin quitar BOM ni normalizar saltos de línea.

| Export                            | SHA-256                                                            |
| --------------------------------- | ------------------------------------------------------------------ |
| F1-10 TXT, 94 378 bytes           | `8a3354af4c1634ad0c97ebccb8be787202e67423d37cc5df548d4137e4549e5f` |
| F1-12 TXT                         | `e0dfa203c4c4ca5bc66b3b4ea3a9e7cced017ef300a9367295836128a5af5729` |
| F4-13 TXT                         | `2574340697c13d20b98f1188e706cae2ccf2147c37a737806d58d7788be96cc2` |
| F4-21 TXT                         | `b1912c884ec384b62944ca366810eaf809fcd257e6bde85744d5c15e7978548d` |
| F1-08 TXT                         | `de5958d5105b636249b97b81b3a1c85bc1dac85149a8670d08cf3a0acba76a63` |
| F1-09 TXT                         | `40346b174cf811d4843368655899ba843c53f54a30804ae51cb01bd0a1dab484` |
| XLSX integral F1-09, 37 362 bytes | `899389e876835f10bc79b905494f1a4fff8b25dcc9cdb1044297ebae85da9462` |
| Matriz F1-10 XLSX, 19 032 bytes   | `43c54e3623dcbe72c72dfec66cf1ad46cb4bc3d0020dd7e619e667d4dc37417e` |
| F4-15 TXT                         | `0eac0adb025c6865c87612896d92cbcb1f43318fc141a871c28b439415d8e6f4` |
| F4-16 TXT                         | `2ee493c06ef3fc6f0ef484b3b0e72aff466ebc095335b34c4b1cd7e1a0d659d2` |
| F4-17 TXT                         | `7adf62fb8b34d55bb92ac675d4c868e6e39dfee39b61629957675ca716225766` |
| F5-09 TXT                         | `cc37a9c23e41e43374a0da6b0842215082612698addc1a14cc3cbff417dc9e75` |
| F5-10 TXT                         | `06b2bd8607efa036575d99f3972b6e64d4aff87590e904bbc891d265a66c9c62` |
| F5-12 TXT                         | `861d2c482adf7792d33405f29b1d79976325be3a836b55caf598dc2369c09947` |
| F5-13 TXT                         | `c72cea808f868ef90c26b841b183e4811d1890474a093d3fd3e38d286edab49d` |
| F1-11 TXT                         | `6f39fd5a2ba9b3a55575bbf2e00390f0a066345eb309b55bbd8ba37700b8460d` |
| F1-13 TXT                         | `cabb799eeb5991046cb07e4e2983cffed2507dae80bf809719964162d29cf7a3` |

El HTML F1-10 produjo hashes distintos en dos consultas:
`a064e6a7d0df578fa58c3a31bb0b6a2a4549f5665510daa31455e99581108437` y
`cb5556cdd06652a63d0af77b4bdc8478455e2225da27945c32797fc26521c972`.
No se presenta su serialización como estable. En la segunda se recontaron todos
los elementos, incluidos los anidados. El TXT coincide con el hash registrado
en la auditoría del 2026-10-27.

## 3. Contraste obligatorio y conteo 82 frente a 83

### 3.1 Qué está fijado y qué sigue abierto

| Fuente                     | Requisito N comprobado                                                                                                                                               | No autoriza deducir                                                                                |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| F1-10 §16.37–38            | `price_version.price_book_id` y `tax_rule_version.tax_profile_id`: FK UUID no nulas a destinos nombrados.                                                            | Campos, lifecycle ni unicidades de los padres sin ficha.                                           |
| F1-10 §16.18–19            | `valuation_entry.costing_policy_id` y `expense.category_id`: UUID, FK no nulas; política versionada y categoría ExpenseCategory.                                     | Nombre SQL definitivo, scope por item, catálogo inicial o política contable de gasto.              |
| F1-10 §16.60; §9           | `plan_version.plan_id` FK no nula a `plan(id)`; versión comercial publicada no cambia retroactivamente.                                                              | Plan por customer/tenant, tarifas, cuotas ni dependencia remota de Free.                           |
| F1-12 §2 y §4              | Preservar UUID, exactitud, snapshots; costo de partida de compra nullable; no usar JSON para sustituir FK.                                                           | Que las cinco entidades ya existan en Java o deban poblarse automáticamente con valores supuestos. |
| F4-13 §7–10 y ratificación | Promedio ponderado móvil por Branch/InventoryItem; ledger físico y económico separados; STRICT normal; excepción ALLOW_NEGATIVE explícita produce costo provisional. | FIFO configurable v1, redondeo fiscal universal o convertir costo desconocido en cero.             |
| F4-21 §1–3, §5–7           | Free offline; SQLite autoritativo; snapshot fiscal/costo exacto; Control Plane no obligatorio; cambios de invariantes por ADR y actualización del freeze.            | Aprobación de estas nuevas fichas, de Pro/Stripe, ni gates ejecutados.                             |

La matriz complementaria sí propone `costing_policy` y `expense_category` como
nombres SQL. Es un hallazgo adicional a la auditoría: **el destino carece de ficha
canónica**, no de toda mención posible en el expediente. Se adoptan aquí solo
como nombres **P**, coherentes con esa matriz **C**. No hay dos tablas por el
hecho de existir nombres PascalCase y snake_case.

### 3.2 Recuento y explicación editorial

Procedimiento sobre F1-10 TXT: buscar encabezados con
`(?m)^[ \t]*((?:16|17)\.\d+) ([a-z][a-z0-9_]*)`, admitir tabulación inicial,
comprobar nombres únicos y bloques con encabezado `Campo`.

| Comprobación                               | Resultado de esta consulta                                                                                            |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| §16.1–67 / §17.1–15                        | 67 + 15, sin huecos: **82 definiciones**.                                                                             |
| Nombres distintos / bloques `Campo`        | **82 / 82**.                                                                                                          |
| Distribución según contextos de las fichas | 59 Local, 9 Central, 14 Control Plane; concuerda con la auditoría.                                                    |
| Elementos HTML `table`, contando anidados  | **83**; uno sin contenido textual de definición, después de §16.7 `cash_register`.                                    |
| Estructura HTML                            | 68 tablas de nivel exterior y 15 anidadas; contar solo nivel exterior daría 68, incorrecto como inventario de fichas. |

**Hecho comprobable:** la cantidad de objetos de formato HTML difiere de las
fichas definidas. **Hipótesis, no causa demostrada:** un conteo editorial de esos
83 objetos pudo originar la cifra declarada por F1-10/F4-21. No hay bitácora,
fórmula, comentario del autor ni historial inspeccionado que demuestre ese origen;
el elemento vacío no prueba intención editorial. No llamar «errata corregida» a
esta observación ni escoger una entidad para ocupar el número 83.

Tampoco `87 entidades lógicas − 82 fichas = cinco dependencias` demuestra
correspondencia de conjuntos. El inventario lógico contiene opcionales y el
físico contiene ampliaciones de paridad. Haría falta un crosswalk completo por
entidad, no una resta. Si se aprobaran cinco tablas nuevas sin otros cambios,
la aritmética sería 87 fichas (63 Local, 9 Central, 15 Control), **solo escenario
P**, no conteo vigente, reconciliación lógica ni meta para «hacer cuadrar» 83.

## 4. Convenciones del diccionario propuesto

### 4.1 Tipos, NULL y defaults

- **N:** F1-10 §2–3, F1-12 §4 y F4-13 ratificado excluyen REAL/float
  autoritativo, FK entre bases y regeneración de UUID. SQLite local usa STRICT.
- **P:** UUID local `TEXT` canónico; PostgreSQL `UUID`. Elegir TEXT descarta BLOB
  para este candidato, no declara cerrada toda alternativa histórica.
- **P:** instantes local `TEXT` ISO-8601 UTC, representación fija validada;
  PostgreSQL `TIMESTAMPTZ`. No confiar en orden lexicográfico de formatos mixtos.
- **P:** texto/estados `TEXT` en ambos; catálogos con CHECK explícito tras revisar
  transiciones. `row_version` usa `INTEGER` / `BIGINT`, >=0, default 0, CAS
  checked sin wraparound. Es concurrencia, no número de versión de negocio.
- En las tablas, `—` significa **sin default SQL**: valor obligatorio de comando
  validado o import. `app` es asignación por aplicación en UTC, no default SQL.
  `NULL` es default nulo explícito. No hay moneda, jurisdicción, precio, tasa,
  método de costeo o estado de aprobación inferidos por default.
- **P:** no almacenar decimales en estos cinco padres: precios, tasas y valores
  siguen en sus versiones/hechos. Allí rigen `TEXT` decimal canónico local y
  `NUMERIC(p,s)` PostgreSQL, con precisión/escala del contrato revisado y
  validación antes del INSERT; no inventar `p,s` ni permitir redondeo implícito.
- Tipos PostgreSQL de padres locales son equivalencia de representación para
  contratos/laboratorio, **no orden de crear copias autoritativas en Central**.
  Para `plan` la autoridad sí es PostgreSQL Control Plane; SQLite es solo una
  equivalencia eventual, no tabla requerida en el POS.

Tipos lógicos compartidos **P** para leer las fichas por campo: `id` y campos
`*_id` son UUID; `code` es código estable; `name` etiqueta Unicode; `currency`
es Currency ISO-4217; `jurisdiction` clave de jurisdicción; `status` enum de su
lifecycle; `row_version` contador CAS; `policy_version` revisión de negocio;
`method` método de costeo; `calculation_contract_version` identificador de
contrato; `created_at`/`updated_at`/`valid_from`/`effective_from` son instantes UTC.
Las columnas SQLite/PostgreSQL de cada ficha indican el tipo físico propuesto;
no confundir UUID lógico con TEXT local sin validación.

### 4.2 Reglas comunes propuestas de integridad

Para los padres organization-owned: PK `id`, FK `organization_id`→organization,
clave candidata `(organization_id,id)`; nunca `UNIQUE(organization_id)` para
codificar cuotas. Para branch-owned: añadir FK
`(organization_id,branch_id)`→branch y clave
`(organization_id,branch_id,id)`. La estrategia de scope es **N** por F4-15 §5;
su materialización en estos padres faltantes es **P**.

`code`/`name` son **P** donde se agregan: código estable ASCII normalizado
(trim + mayúsculas), no vacío; nombre Unicode recortado no vacío. Normalizar en
Rust y verificar forma almacenada, sin suponer iguales `NOCASE`, locale o
casefold de SQLite/PostgreSQL. Longitudes máximas quedan **Q**, sin cifras
supuestamente normativas. Unicidades por código se precisan por dependencia.

**P:** `created_at`/`updated_at` no nulos, `app`, actualización con CAS y auditoría;
`updated_at >= created_at`, sin usar reloj como versión. UUID, scope y código
no se reasignan. Borrado referenciado: RESTRICT, sin cascada destructiva ni
SET NULL que pierda trazabilidad; desactivación conserva historia. No se propone
hard delete operativo aun sin referencias, hasta fijar retención.

**N:** single writer por scope y backend local autoritativo (F4-21 §2).
**P:** comandos administrativos con permiso de dominio, `expectedVersion`,
commandId/digest y audit/event/outbox transaccional según corresponda; un código
único no sustituye deduplicación de comando. No se agregan columnas de writer,
usuario aprobador o secretos para duplicar identidad/auditoría ya modeladas.
El detalle de permisos todavía requiere catálogo, no nombres de permiso inventados.

Índices marcados «acceso» son **P**, sujetos a consultas/EXPLAIN; los asociados a
PK/UK/FK garantizan integridad o búsquedas referenciales. Evitar duplicar un índice
ya cubierto por prefijo de PK/UK. Se especifican también índices en hijos: son
requisitos propuestos para futura reconciliación, **no cambios a esas fichas**.

## 5. `price_book` — PROPUESTO NO APROBADO

### Significado, evidencia y scope

**N:** es el padre requerido por `price_version` en F1-10 §16.37; el precio
publicado/histórico no se recalcula por cambiar catálogo (F4-21 §1–2).
**C:** F5-10 §3, §8, §10 y F1-08 §16 lo describen como lista de precios de
Organization; una lista activa Free y overrides branch futuros son propuestas.
XLSX F1-09: `Entidades` fila 31; `Campos` filas 160–163:
identidad, organización, moneda y estado. La matriz F1-10 `Entidades` fila 31
menciona org/branch **a revisar**, no aprueba un scope mixto.

**P:** organization-owned, sin `branch_id` en v1; catálogo de precios base,
no importe por producto ni autoridad tributaria. Un único writer administrativo
para ese catálogo; la Branch es contexto de venta/autorización, no dueña del
precio base. No confundir `price_book` con `plan` comercial de HiQNet.

### Campos propuestos

| Campo             | SQLite STRICT / PostgreSQL | Clave            | NULL | Default | Constraint / semántica                                    | Base  |
| ----------------- | -------------------------- | ---------------- | ---- | ------- | --------------------------------------------------------- | ----- |
| `id`              | TEXT / UUID                | PK               | NO   | —       | Identidad estable del libro                               | C + P |
| `organization_id` | TEXT / UUID                | FK, UK compuesta | NO   | —       | Organization dueña; no transferible                       | C + P |
| `code`            | TEXT / TEXT                | UK con org       | NO   | —       | Código normalizado estable                                | P     |
| `name`            | TEXT / TEXT                | —                | NO   | —       | Nombre visible no vacío                                   | P     |
| `currency`        | TEXT / TEXT                | —                | NO   | —       | ISO-4217 validada; igual a Organization en v1; inmutable  | C + P |
| `status`          | TEXT / TEXT                | —                | NO   | —       | ACTIVE/INACTIVE; alta explícita, sin activación implícita | C + P |
| `row_version`     | INTEGER / BIGINT           | —                | NO   | 0       | CAS >=0                                                   | P     |
| `created_at`      | TEXT UTC / TIMESTAMPTZ     | —                | NO   | app     | Auditoría temporal                                        | P     |
| `updated_at`      | TEXT UTC / TIMESTAMPTZ     | —                | NO   | app     | Regla común §4                                            | P     |

**Claves/índices P:** UK `(organization_id,id)` y
`(organization_id,code)`; índice acceso `(organization_id,status)` solo si lo
justifican consultas. Hijo: FK propuesta
`price_version(organization_id,price_book_id)` al padre y FK scope de producto;
requiere añadir/reconciliar scope en la ficha de ese hijo. Índice candidato
`(price_book_id,product_id,effective_from)` y UK
`(price_book_id,product_id,version)`, no versión única para todo el libro.

**Cardinalidad/version/inmutabilidad P:** Organization 1→0..N libros; libro
1→0..N PriceVersion; cada versión tiene exactamente un libro y un producto del
mismo org. La no superposición se evalúa por libro/producto y ventana, no por
libro completo ni solo con UNIQUE de fechas. F1-10 §16.37 ya pide precio publicado
inmutable; no convertir `row_version` del padre en versión económica. Nombre y
estado pueden cambiar auditadamente; moneda/identidad no. Una desactivación evita
nuevas selecciones pero no invalida snapshots. Publicación/retiro y cierre de
ventanas del hijo requieren reconciliar la inmutabilidad con `effective_to`;
no se autoriza editar versiones publicadas para encajar este candidato.

**Free P:** resolver exactamente una lista activa en el comando local; guard
transaccional de activación, no unique permanente por org ni llamada remota.
Migración de `products.unit_price` (F1-12 §2) necesita libro/IDs y fecha de versión
inicial explícitos en manifest; nada de UUID aleatorio en cada retry o fecha
histórica inventada.

**Q-PB:** ¿ratificar una lista org activa v1 y dejar overrides fuera? ¿Reglas para
cambio de moneda de Organization con precios/historia? ¿Fecha/identidad de libro
inicial en imports y política de vigencia/publicación? ¿Qué longitud/código y
comportamiento ante precio ausente se autorizan? Sin esas respuestas no dar por
resuelto el import de catálogo ni el lookup de checkout.

**Tests propuestos PB:** FK cross-org rechazada incluyendo producto; moneda
incompatible rechazada; doble activación serializada; dos precios de productos
distintos no colisionan, ventanas solapadas del mismo producto sí se rechazan;
cambio de nombre/estado no altera venta; import reintentado no duplica libro ni
versión inicial y preserva `NUMERIC(19,4)` exacto; Free cotiza sin Control Plane.

## 6. `tax_profile` — PROPUESTO NO APROBADO

### Significado, evidencia y scope

**N:** padre no nulo de TaxRuleVersion (F1-10 §16.38); clasificación explícita,
reglas/snapshots históricos y rounding versionado (F4-13 §5–6, F4-17 §3–4,
ratificados por F4-21). Cero, exento y no objeto no son equivalentes.
**C:** F5-10 §6 y F1-08 §16: configuración tributaria por jurisdicción,
organization-owned. XLSX F1-09 `Entidades` fila 37, `Campos` 196–200 aporta
`id`, `organization_id`, `jurisdiction`, `status`, `valid_from`.

**P:** Organization es dueña del perfil local, no HiQNet ni el país entero.
Jurisdicción identifica el ámbito de configuración, no prueba aprobación fiscal.
El padre agrupa reglas; no duplica `rate_exact`, clasificación ni `tax_mode` del
precio. Un writer administrativo autorizado, con revisión fiscal cuando aplique.

### Campos propuestos

| Campo             | SQLite STRICT / PostgreSQL | Clave            | NULL | Default | Constraint / semántica                                               | Base  |
| ----------------- | -------------------------- | ---------------- | ---- | ------- | -------------------------------------------------------------------- | ----- |
| `id`              | TEXT / UUID                | PK               | NO   | —       | Identidad estable del perfil                                         | C + P |
| `organization_id` | TEXT / UUID                | FK, UK compuesta | NO   | —       | Organization dueña                                                   | C + P |
| `code`            | TEXT / TEXT                | UK con org       | NO   | —       | Código estable normalizado                                           | P     |
| `name`            | TEXT / TEXT                | —                | NO   | —       | Etiqueta administrativa no vacía                                     | P     |
| `jurisdiction`    | TEXT / TEXT                | —                | NO   | —       | Clave explícita de jurisdicción; catálogo por revisar, no default MX | C + P |
| `valid_from`      | TEXT UTC / TIMESTAMPTZ     | —                | NO   | —       | Inicio habilitado del perfil; no sustituye vigencias de reglas       | C + P |
| `status`          | TEXT / TEXT                | —                | NO   | —       | ACTIVE/INACTIVE explícito                                            | C + P |
| `row_version`     | INTEGER / BIGINT           | —                | NO   | 0       | CAS >=0                                                              | P     |
| `created_at`      | TEXT UTC / TIMESTAMPTZ     | —                | NO   | app     | UTC                                                                  | P     |
| `updated_at`      | TEXT UTC / TIMESTAMPTZ     | —                | NO   | app     | Regla común §4                                                       | P     |

**Claves/índices P:** UK `(organization_id,id)` y
`(organization_id,code)`; acceso `(organization_id,jurisdiction,status)` si se
usa. Hijo: scope explícito y FK `(organization_id,tax_profile_id)`; índice
`(tax_profile_id,effective_from)`, UK `(tax_profile_id,version)` solo bajo la
propuesta de que cada revisión representa una regla completa del perfil. Si un
perfil combina impuestos independientes, revisar esa UK y usar identidad de
regla/tipo explícita antes de materializar; no bloquear combinaciones legítimas.

**Cardinalidad/version/inmutabilidad P:** Organization 1→0..N perfiles, perfil
1→0..N revisiones; no UK por org/jurisdicción que impida tratamientos distintos.
`jurisdiction`/`valid_from` no cambian tras primera regla publicada/uso; nuevo
ámbito requiere otro perfil. Nombre/estado con CAS; reglas y snapshots usados
inmutables por baseline. Desactivar no cambia reimpresión/refund. La regla
seleccionada debe estar dentro del período habilitado del perfil y vigente en
el contexto de operación; no aplicar automáticamente la última versión.

**Brecha separada:** F1-08/F5-10 nombran `ProductTaxAssignment`, pero no aparece
como ficha del físico contado. La FK padre propuesta no resuelve cómo producto,
operación y jurisdicción seleccionan perfil/reglas ni cómo se representan varios
impuestos. No se añade otra tabla por mención, ni se traslada esa relación a JSON.

**Free:** cálculo local con reglas/versiones configuradas; sin SAT/PAC/Control
Plane durante checkout (**N**). No seed de «todo IVA 16%», exención global ni
HALF_UP universal (**P: no introducir esos defaults**). La ausencia de datos
legados de impuestos requiere una política de import explícita, no fabricar
historia fiscal. CFDI/PAC LATER no elimina la necesidad de snapshot reproducible.

**Q-TP:** ¿catálogo de jurisdicciones y fuente de configuración autorizada?
¿Un perfil es una regla o un conjunto de tributos? ¿Cuál es la selección por
producto/operación y su vigencia? ¿Cómo mapear ventas legadas sin detalle fiscal?
¿Rounding policy y comportamiento de quote con configuración incompleta? Ninguna
respuesta se atribuye al fiscalista o al titular sin registro explícito.

**Tests propuestos TP:** cross-org, perfil aún no vigente y resolución ambigua
rechazados; 0/exento/no objeto distintos con igual importe; NET/GROSS sin doble
cobro; cambio de versión entre quote/checkout detectado; reimpresión/refund y
restore usan regla/rounding/snapshot originales; varios impuestos no colisionan
por unicidad excesiva; sin perfiles implícitos en import.

## 7. `CostingPolicy` → `costing_policy` — PROPUESTO NO APROBADO

### Significado, evidencia y scope

**N:** F1-10 §16.18 requiere UUID de política versionada en ValuationEntry;
F4-13 §7–10 y ratificación fijan promedio móvil por Branch/InventoryItem,
provisionalidad y hechos compensatorios, sin reescribir ledger.
**C:** XLSX F1-09 `Entidades` fila 50 / `Campos` 260–264:
`id`, `branch_id`, `method`, `effective_from`, `status`. F1-08 §17 dice
Branch/Item pero no contiene campo `item_id` en esa fila. La matriz F1-10
`Entidades` fila 45 propone `costing_policy`; no es ficha física canónica.

**P:** política branch-owned, una secuencia de revisiones para toda la Branch;
el promedio se calcula **separadamente por Branch/Item**, no mezclando ítems.
No agregar `item_id` solo por esa frase: una política por item es alternativa
pendiente, no consecuencia necesaria del algoritmo. Writer de la Branch para
publicar políticas y asentar valor; nada de editor remoto concurrente offline.

### Campos propuestos

| Campo                          | SQLite STRICT / PostgreSQL | Clave                                  | NULL | Default | Constraint / semántica                                             | Base                    |
| ------------------------------ | -------------------------- | -------------------------------------- | ---- | ------- | ------------------------------------------------------------------ | ----------------------- |
| `id`                           | TEXT / UUID                | PK                                     | NO   | —       | Identifica esta revisión de política                               | C + P                   |
| `organization_id`              | TEXT / UUID                | FK, parte UK                           | NO   | —       | Scope explícito F4-15                                              | P sobre N de scope      |
| `branch_id`                    | TEXT / UUID                | FK compuesta, parte UK                 | NO   | —       | Branch del mismo org                                               | C + P                   |
| `policy_version`               | INTEGER / BIGINT           | UK con org/branch                      | NO   | —       | >=1; versión de negocio, no CAS                                    | P                       |
| `method`                       | TEXT / TEXT                | —                                      | NO   | —       | Solo MOVING_WEIGHTED_AVERAGE en v1; comando explícito              | C + P sobre N de método |
| `effective_from`               | TEXT UTC / TIMESTAMPTZ     | UK con org/branch                      | NO   | —       | Activación explícita; no fecha histórica supuesta                  | C + P                   |
| `calculation_contract_version` | TEXT / TEXT                | referencia de contrato, no FK ficticia | NO   | —       | Versión implementable de algoritmo/precisión/rounding, sin default | P                       |
| `status`                       | TEXT / TEXT                | —                                      | NO   | DRAFT   | DRAFT/PUBLISHED/RETIRED propuestos                                 | C + P                   |
| `row_version`                  | INTEGER / BIGINT           | —                                      | NO   | 0       | CAS de lifecycle, distinto de policy_version                       | P                       |
| `created_at`                   | TEXT UTC / TIMESTAMPTZ     | —                                      | NO   | app     | UTC                                                                | P                       |
| `updated_at`                   | TEXT UTC / TIMESTAMPTZ     | —                                      | NO   | app     | Lifecycle auditable                                                | P                       |

`calculation_contract_version` propone un identificador estable de contrato
resoluble por el runtime, que fije escalas/rangos/rounding de costo y tratamiento
de Q=0/residuo/provisionalidad. No es JSON de reglas arbitrarias ni una FK a otra
tabla inventada. Un identificador desconocido impide publicar/asentar valor.
Su contenido concreto sigue **Q**; crear el campo no cierra la política.

**Claves/índices P:** UK `(organization_id,branch_id,id)`,
`(organization_id,branch_id,policy_version)` y
`(organization_id,branch_id,effective_from)`. Búsqueda por scope/fecha cubierta
por esta última. Hijo ValuationEntry: scope explícito derivado y comprobado
contra InventoryMovement y FK compuesta a la política; índice
`(costing_policy_id,created_at)` solo si lo usan auditorías. No UK global sobre
`inventory_movement_id`: compensaciones pueden requerir varias entradas.

**Cardinalidad/version/inmutabilidad P:** Branch 1→0..N revisiones; cada asiento
apunta a exactamente una revisión; política 1→0..N asientos. Publicación congela
método, contrato, scope y fecha. Retirar solo lifecycle/auditoría; la referencia
histórica sigue válida. Sin `effective_to` editable: resolver revisión publicada
más reciente con inicio <= instante del movimiento, con orden/fecha controlados
y sin retroactividad sobre asientos ya creados. La revisión usada se guarda,
no se vuelve a resolver al consultar historia. El guard transaccional valida
versiones/fechas monotónicas y evita dos publicaciones competidoras. Los detalles
de sucesión/retiro y reconstrucción siguen pendientes de aprobación.

**Free:** método ya ratificado, no selector FIFO ni dependencia comercial.
Activar una política inicial requiere parámetros/versiones explícitos y trazables;
no fabrica valoración histórica del Java que carece de motor equivalente.
`organization.stock_negative_policy` no se duplica aquí ni se cambia por defecto;
se valida junto a la política aplicable. ALLOW_NEGATIVE explícito no equivale a
costo confirmado ni autoriza margen definitivo.

**Q-CP:** ¿ratificar revisión branch-wide aplicada por item, o requerir override
por item con otra clave/cardinalidad? ¿Contrato numérico, residuo Q=0 y fecha de
sucesión/retiro? ¿Política histórica para recepciones Java sin costo? ¿Cómo se
representan costo pendiente/ajustes y la selección de política en cutover?

**Tests propuestos CP:** FK de política de otra Branch/org rechazada;
publicaciones concurrentes/retroactivas rechazadas; usar política publicada
inmutable aun tras retiro; 10×20 + 10×30 ⇒ promedio 25, aislado por item y Branch;
Q=0/Q<0 sin división ficticia; provisional no confirma margen; retry de receipt
no duplica valor; compensación agrega asiento y preserva original; contrato
numérico desconocido o overflow revierte toda la transacción.

## 8. `ExpenseCategory` → `expense_category` — PROPUESTO NO APROBADO

### Significado, evidencia y scope

**N:** Expense tiene `category_id` UUID FK no nula, Organization y Branch opcional
(F1-10 §16.19, F4-15 §5.1). Gastos/devengos y pagos permanecen separados; CASH
produce un solo egreso, no compra/merma automáticas (F4-21 §1; F1-10 §6).
**C:** F5-09 §5, §9–10: categoría de gasto operativo, no de Product ni costo
capitalizable. XLSX F1-09 `Entidades` fila 55 / `Campos` 286–289:
`id`, `organization_id`, `name`, `status`. Matriz F1-10 `Entidades` fila 55 propone
nombre SQL `expense_category`.

**P:** catálogo organization-owned compartido por gastos org/branch; sin Branch
propia en la categoría. Una categoría no concede permiso para pagar ni decide
tratamiento fiscal/contable, inventario o método de caja. Writer administrativo
del catálogo de la Organization, distinto de la autorización de cada gasto.

### Campos propuestos

| Campo             | SQLite STRICT / PostgreSQL | Clave            | NULL | Default | Constraint / semántica         | Base  |
| ----------------- | -------------------------- | ---------------- | ---- | ------- | ------------------------------ | ----- |
| `id`              | TEXT / UUID                | PK               | NO   | —       | Identidad estable              | C + P |
| `organization_id` | TEXT / UUID                | FK, UK compuesta | NO   | —       | Organization dueña             | C + P |
| `code`            | TEXT / TEXT                | UK con org       | NO   | —       | Código estable normalizado     | P     |
| `name`            | TEXT / TEXT                | —                | NO   | —       | No vacío; etiqueta descriptiva | C + P |
| `status`          | TEXT / TEXT                | —                | NO   | —       | ACTIVE/INACTIVE explícito      | C + P |
| `row_version`     | INTEGER / BIGINT           | —                | NO   | 0       | CAS >=0                        | P     |
| `created_at`      | TEXT UTC / TIMESTAMPTZ     | —                | NO   | app     | UTC                            | P     |
| `updated_at`      | TEXT UTC / TIMESTAMPTZ     | —                | NO   | app     | Regla común §4                 | P     |

**Claves/índices P:** UK `(organization_id,id)` y `(organization_id,code)`;
no UK por nombre visible. Índice acceso `(organization_id,status)` si se usa.
Hijo Expense: FK `(organization_id,category_id)` al padre; índice
`(organization_id,category_id,posted_at)` para reporte si lo justifican queries.
La FK de Branch opcional verifica pertenencia cuando no es NULL; no permite
categoría de otra org porque esa FK usa org/category no nulos.

**Cardinalidad/version/inmutabilidad P:** Organization 1→0..N categorías;
categoría 1→0..N gastos; cada gasto exactamente una categoría. Sin jerarquías,
centros de costo, traducciones, cuentas contables ni flag de capitalización
inventados. Se propone código/identidad estables, nombre/estado editables con CAS.
Desactivar bloquea asignaciones nuevas, no impide pagar gasto ya POSTED ni borra
historia. No se reclasifica un gasto POSTED por editar el catálogo.

**Brecha histórica:** el físico no define snapshot de nombre/código de categoría
en Expense. Este candidato permite reportar identidad estable y etiqueta actual;
no promete reconstruir la etiqueta que existía al devengo. Si eso es requisito,
se debe aprobar snapshot/versionado adicional antes de dar conformidad histórica.

**Free:** categorías y gastos locales sin licencia remota. No seed automático
«otros» para satisfacer FK ni mezclar `category` de productos con esta entidad.
No hay tabla AS-IS de gasto que permita afirmar un import directo 1:1.

**Q-EC:** ¿identidad estable basta o hace falta etiqueta histórica? ¿Se admiten
reclasificaciones antes/después de POSTED y cómo se auditan? ¿Catálogo inicial
explícito o alta por el titular? ¿Scope org compartido o categorías branch
específicas? Ninguna categoría supone deducibilidad fiscal aprobada.

**Tests propuestos EC:** cross-org rechazado aun con Expense.branch NULL;
desactivar no borra ni altera pagos existentes; no duplicación de código
normalizado, nombre repetido permitido; gasto POSTED no se reclasifica por
renombrar categoría; CASH retry produce una salida, BANK ninguna; sin categoría
no se postea ni se crea una implícitamente.

## 9. `plan` — PROPUESTO NO APROBADO

### Significado, evidencia y scope

**N:** F1-10 §16.60 exige padre `plan(id)` para PlanVersion; §9 y F4-16 §2–5
ratificado separan catálogo versionado/entitlements, contrato, licencia y RBAC.
**C:** F5-12 §5, §7 y §11: Plan es identidad comercial, PlanVersion una versión
de condiciones/capacidades; Plan 1:N PlanVersion. XLSX F1-09 `Entidades` fila 77 /
`Campos` 390–392: `id`, `name`, `status`. Matriz F1-10 `Entidades` fila 79 tiene
scope genérico tenant/business «según contexto»: no establece ownership por cliente.

**P:** catálogo global de producto HiQNet en Control Plane, no Organization,
Branch, Customer ni tenant operacional. Operadores comerciales autorizados son
los writers; un cliente contrata una versión, no modifica la definición del plan.
Free/Pro/Enterprise son ediciones/capacidades, no permisos de usuario ni esquemas
SQL diferentes. La separación global frente a planes privados por contrato sigue
requiriendo confirmación explícita.

### Campos propuestos

| Campo         | PostgreSQL autoridad / SQLite STRICT equivalente, no requerida | Clave     | NULL | Default | Constraint / semántica                                                   | Base  |
| ------------- | -------------------------------------------------------------- | --------- | ---- | ------- | ------------------------------------------------------------------------ | ----- |
| `id`          | UUID / TEXT                                                    | PK        | NO   | —       | Identidad comercial estable                                              | C + P |
| `code`        | TEXT / TEXT                                                    | UK global | NO   | —       | Código estable normalizado; sin catálogo Free/Pro hardcodeado como CHECK | P     |
| `name`        | TEXT / TEXT                                                    | —         | NO   | —       | Nombre visible no vacío; no define términos                              | C + P |
| `status`      | TEXT / TEXT                                                    | —         | NO   | —       | ACTIVE/INACTIVE explícito, catálogo propio del padre                     | C + P |
| `row_version` | BIGINT / INTEGER                                               | —         | NO   | 0       | CAS administrativo, no PlanVersion.version                               | P     |
| `created_at`  | TIMESTAMPTZ / TEXT UTC                                         | —         | NO   | app     | UTC                                                                      | P     |
| `updated_at`  | TIMESTAMPTZ / TEXT UTC                                         | —         | NO   | app     | Regla común §4                                                           | P     |

**Claves/índices P:** UK `(code)`; sin FK a Organization/Customer/tenant local;
acceso por status solo si hay consulta. Mantener FK `plan_version.plan_id` y
UK `(plan_id,version)` ya descritas por F1-10 §16.60; esa UK cubre búsqueda por
plan. Subscription/License referencian PlanVersion, no el «plan actual».
No colocar precio, moneda, meses, cuota de sucursales, provider_price_id,
grants ni grace en el padre para cambiar términos al renombrarlo.

**Cardinalidad/version/inmutabilidad P:** Plan 1→0..N versiones, incluyendo
identidad todavía sin versión publicada; cada PlanVersion exactamente un Plan.
ID/código estables; nombre/estado editables auditadamente, solo identidad comercial.
PUBLISHED de PlanVersion es inmutable por **N**; nuevas condiciones se publican
en otra versión. P propone que INACTIVE impida nuevas publicaciones/contratación,
no revoque versiones/grants históricos ni suscripciones vigentes. La etiqueta
comercial histórica, si es contractual, debe quedar en versión/documento firmado;
no se afirma resuelta por `plan.name` mutable.

**Free:** no depende de fila Plan, Customer, Subscription ni License remotas
(**N**, F1-10 §9; F4-16 ratificación; F4-21 §3–4). No sembrar suscripción falsa
ni replicar catálogo CP al POS para permitir checkout. Los valores de Free y
protecciones base viven en política local confiable; los límites comerciales no
son FK ni UNIQUE sobre sucursales históricas. Pro/Enterprise, precio, proveedor,
grace y activación comercial permanecen LATER/por aprobar donde corresponda.

**Q-PL:** ¿catálogo global o planes privados Enterprise con scope distinto y
política de código? ¿Nombre histórico contractual en PlanVersion o documento?
¿Qué significa desactivar el padre frente a contratar una versión publicada?
¿Quién aprueba/pública condiciones y qué evidencia se exige? No se asignan
aprobaciones ni se fija precio/cuota/plazo en este candidato.

**Tests propuestos PL:** PlanVersion huérfana rechazada; versión repetida dentro
del mismo plan rechazada, misma versión en otro plan permitida; publicación
inmutable salvo lifecycle de retiro auditado según contrato; renombrar/desactivar
Plan no cambia grants/contratos ni RBAC; operador no autorizado no publica;
Free checkout sin Control Plane ni filas remotas; downgrade conserva historia y
protecciones core. No se acepta como test un pago o precio comercial ficticio.

## 10. Dependencias que esta propuesta NO resuelve

1. **Compras/paridad:** F1-12 §2 exige `unit_cost` nullable; F1-10 §16.40–41
   exige costo/total no nulos y costo de recepción. F4-13 no transforma NULL en
   cero ni ofrece costo histórico faltante. Crear CostingPolicy no solventa
   `line_total_exact`, receipt cost, moneda o snapshots ausentes de compras.
   Se conserva el bloqueo de import afectado del ADR-0021.
2. **Snapshots y scopes:** FKs compuestas propuestas requieren reconciliar
   campos en `price_version`, `tax_rule_version`, `valuation_entry` y otros
   hijos. No se afirma que el físico actual ya tenga todo el scope necesario.
3. **Cobertura fiscal/costo:** selección Product→TaxProfile, combinación de
   impuestos, registro de política de rounding y destinos de costo pendiente /
   snapshots de costo siguen requiriendo decisión. No se añaden tablas opcionales
   ni JSON genérico para aparentar cierre.
4. **Import y seed:** algunas dependencias son nuevas, sin tabla Java 1:1.
   Separar filas históricas preservadas, configuración inicial explícita y
   fixtures sintéticas. Manifest versionado con IDs deterministas/persistidos,
   procedencia y regla aprobada; replay idéntico no crea datos nuevos y payload
   diferente no sobrescribe configuración usada.
5. **Gobernanza:** cinco fichas candidatas no corrigen F1-10, F4-21 ni ADR-0021
   por sí mismas. La fuente canónica exige tabla nativa por campo; este Markdown
   es material para revisión, no esa publicación ni autoridad sustitutiva.

## 11. Validación requerida tras una eventual aprobación

No hay tests implementados ni ejecutados por este incremento documental. Los
casos PB/TP/CP/EC/PL anteriores son **propuestas de cobertura**, no IDs normativos
nuevos ni gates verdes. La futura validación debe separar invariantes de negocio
(N) de elecciones físicas aprobadas posteriormente (P).

| Nivel / gate existente           | Cobertura mínima futura                                                                                                                           | Evidencia esperada, no generada aquí                                                          |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Diccionario/decisión             | Respuesta a Q por dependencia; tipos, scopes, lifecycle, defaults e inmutabilidad revisados; crosswalk y control de cambios                       | Registro explícito de decisión/versión/responsables, sin atribución de aceptación previa      |
| Unit/domain                      | Normalización, validación de UUID/UTC/moneda, CAS, selección vigente, contratos numéricos, autorización de writer y publicación                   | Casos positivos, negativos y de borde; ninguna coerción silenciosa                            |
| SQLite integración               | STRICT real, foreign_keys=ON por conexión antes de transacción, PK/UK/FK/CHECK/NULL/defaults; referencias cross-org/branch; retiro y concurrencia | Catálogo/constraints más intentos de escritura inválida y `foreign_key_check` limpio          |
| PostgreSQL integración           | `plan`/versiones Control Plane; representaciones exactas y ausencia de FK cross-DB; permisos administrativos                                      | SQL de laboratorio futuro y pruebas reales, no equivalencia inferida de SQLite                |
| F1-13 DDL13-01..04               | Clean/upgrade por raíz/contexto aplicable; no catalogar plan CP como requisito de clean Local                                                     | Esquema versionado y tracking/checksums SQLx; no suma de conteos para justificar autoridad    |
| F1-13 failure/restore y AS-10    | Fallo antes/durante commit no deja config/hechos parciales; backup/restore conserva referencias, versiones, historia y outbox                     | Ensayo de fallo/restauración y comparación de digests                                         |
| PAR-01..09; AS-01..08 aplicables | Conteos explicados, UUID, FK/ownership, money/quantity, snapshots, append-only, idempotencia; configuración nueva identificada por separado       | Manifest/crosswalk sin omitir filas ni inventar costo/impuesto; asserts de NULL frente a cero |
| F4-21 IMP-06 / F4-16 gates       | Free offline sin CP, guard de lista activa, plan no sustituye RBAC, downgrade no destructivo                                                      | Tests locales de flujo y autorización; Pro sync/billing no se declara habilitado              |

Para inmutabilidad, intentar UPDATE/DELETE prohibidos con la ruta de dominio y
con el rol DB previsto; comprobar qué protege el motor, qué depende de permisos /
triggers y qué solo de dominio. Un CHECK no puede garantizar sumas, pertenencia
entre filas o ventanas concurrentes por sí solo. No prometer resistencia a un
administrador con control irrestricto del archivo SQLite.

## 12. Cierre del incremento documental

**Realizado:** lectura de ADR/auditoría y fuentes enlazadas; recuperación de campos
lógicos del XLSX; contraste de negocio/scopes; hashes y recuento TXT/HTML;
propuesta separada por dependencia y matriz de preguntas/tests. Se confirmó la
rama y un árbol inicialmente limpio. No se ejecutaron migraciones, importaciones,
pruebas Java/Rust ni operaciones productivas; no se cambiaron los documentos
anteriores ni se copiaron exports al repositorio.

**Validación documental ejecutada:** comprobación de enlaces locales, longitud
de hashes, presencia de los cinco diccionarios, ausencia de sentencias DDL y
espacios finales; `git diff --no-index --check` del archivo nuevo sin incidencias.
Los cambios concurrentes aparecidos en otros archivos durante la investigación
no pertenecen a este write set y no fueron modificados ni revertidos.

**Pendiente de titular/revisores:** confirmar las opciones P y responder Q;
registrar decisión y control de cambios en fuentes canónicas antes de implementar.
El baseline Free existente no se reabre por tests aún pendientes, pero estas
brechas materiales tampoco se declaran resueltas por la existencia de este archivo.
**Full completion 0b, import afectado y cutover no se dan por desbloqueados.**
