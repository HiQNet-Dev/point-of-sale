# Propuesta de ficha física Local — `audit_event`

**Estado: PROPUESTA TÉCNICA, SIN APROBACIÓN NORMATIVA.**

Este documento prepara la revisión de la ficha Local cuyo nombre aparece como
`audit_event` en F1–10 §17.2. No modifica F1–10..13/F4–14/F4–21, no altera
ADR-0021/CC-0021 y **no autoriza DDL, migración SQLx ni uso productivo**. La
tabla `audit_fixture` de `pos-storage-sqlite` no implementa esta propuesta.

## Autoridad y objetivo

La fuente funcional vigente es Java V7 `audit_events` junto con
`docs/04-security/audit.md` y `docs/02-database/audit-model.md`:

- historia persistente de mutaciones administrativas y de seguridad, no log de
  debug ni volcado de requests;
- append-only impuesto por la base de datos;
- una misma transacción para mutación y hecho de auditoría;
- actor `USER` o `SYSTEM`, isolation por Organization y metadata segura de hasta
  16 KiB;
- sin retención/purge automático todavía.

La nomenclatura Local candidata es singular (`audit_event`), pero debe ser
confirmada por la ficha normativa. No se adopta por analogía el nombre Java
`audit_events`.

## Campos candidatos que la ficha debe decidir

| Campo candidato | Requisito / procedencia | Decisión pendiente |
| --- | --- | --- |
| `id` | UUID estable, PK, generado por aplicación | tipo, representación y generador Local |
| `organization_id` | NOT NULL; todo evento pertenece a una organización | FK y clave padre Local aprobadas |
| `branch_id` | nullable; si existe debe pertenecer a la misma organización | FK compuesta `(branch_id, organization_id)` hacia `branch` |
| `actor_type` | `USER` o `SYSTEM` | `CHECK` cerrado y semántica de procesos internos |
| `actor_user_id` | `USER` lo exige; `SYSTEM` lo prohíbe | FK compuesta same-org hacia el usuario Local aprobado |
| `local_session_id` | correlación local opcional para un USER cuando el mecanismo Local la defina | nombre, FK, lifecycle y obligatoriedad; no usar `token_jti` heredado sin semántica Local |
| `action` | código estable `entity.action`, no vacío | catálogo/límites de longitud |
| `entity_type` / `entity_id` | entidad afectada y UUID cuando aplique | catálogo/límites y nulabilidad aprobados |
| `reason` | nullable; reservado para correcciones, ajustes y refunds | casos y límite de longitud |
| `metadata` | JSON estructurado seguro, máximo 16 KiB serializados | formato SQLite, `CHECK` y validación Rust |
| `occurred_at` | UTC, Clock inyectable, NOT NULL | serialización RFC 3339 y precisión |

La fila no almacenará JWTs, headers `Authorization`, refresh/access tokens,
hashes, contraseñas, secretos, claves privadas ni PII que no sea necesaria.

## Matriz de contrato físico revisable

La siguiente matriz no es DDL ni autorización: convierte la evidencia Java y la foundation Rust en una ficha que el titular normativo debe confirmar. `Aprobado` significa que existe en Java V7; no significa que esté aprobado para Local.

| Campo candidato | Tipo Rust actual | SQLite candidato | NULL | CHECK / FK / índice | Regla de negocio | Procedencia | Estado Local |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `id` | `AuditEventId(EntityId)` | `TEXT` UUID canónico | No | PK; validación UUID igual a agregados | identidad estable generada fuera de DB | Java V7; audit.md | Pendiente de aprobación física |
| `organization_id` | `EntityId` | `TEXT` | No | FK `organization(id)`; prefijo de todos los índices de lectura | todo hecho tiene un único owner | Java V7; audit-model.md | Pendiente |
| `branch_id` | `Option<EntityId>` | `TEXT` | Sí | FK compuesta `(branch_id, organization_id) -> branch(id, organization_id)` | una sucursal opcional nunca cruza org | Java V7; `0001` expone superkey futura | Pendiente |
| `actor_type` | `AuditActor` | `TEXT` | No | `IN ('USER','SYSTEM')` | actor explícito y atribuible | Java V7; audit.md | Pendiente |
| `actor_user_id` | `AuditActor::User` | `TEXT` | Condicional | `USER => NN`; `SYSTEM => NULL`; futura FK compuesta al usuario Local | no hay usuario técnico ficticio | Java V7; audit.md | Bloqueado por usuario Local |
| `local_session_id` | No materializado | `TEXT` candidato | Por decidir | FK y lifecycle por definir | no copiar `token_jti` JWT ni inventar sesiones | F4–21 / propuesta 1D | Bloqueado por Auth Local |
| `action` | `AuditAction` | `TEXT` candidato | No | no vacío tras trim; catálogo/límite a aprobar | código estable de negocio, normalmente `entity.action` | Java `AuditRecord`, V7 | Límite/catálogo pendientes |
| `entity_type` | `AuditEntityType` | `TEXT` candidato | No | no vacío tras trim; catálogo/límite a aprobar | tipo estable, no nombre de clase | Java `AuditRecord`, V7 | Límite/catálogo pendientes |
| `entity_id` | `Option<EntityId>` | `TEXT` | Sí | UUID si existe | entidad principal afectada | Java V7 | Pendiente representación UUID |
| `reason` | `Option<AuditReason>` | `TEXT` candidato | Sí | límite/semántica por aprobar | reservado para ajustes, refunds y correcciones | Java V7 | Pendiente |
| `metadata` | `AuditMetadata` | `TEXT` JSON candidato | Sí | `json_valid`; bytes UTF-8 ≤ 16384; validación Rust previa | JSON seguro, no request dump ni secretos | Java V7; audit.md | Fixture verificado; DDL pendiente |
| `occurred_at` | `AuditOccurredAt` | `TEXT` UTC RFC 3339 candidato | No | validación de formato/precisión a aprobar | reloj inyectado, no reloj del cliente | Java V7; audit.md | Pendiente |

Los índices candidatos se mantienen limitados a las consultas Java comprobadas: `(organization_id, occurred_at DESC, id DESC)`, y, tras confirmar el endpoint Local, compuestos por `branch_id`, `actor_user_id` y `(entity_type, entity_id)`. No se añade particionado, retención, búsqueda externa ni índices especulativos.

## Decisiones cerradas y dependencias abiertas

**Puede revisarse ahora:** UUID de evento; owner obligatorio; branch same-org; actor cerrado USER/SYSTEM; metadata JSON segura de 16 KiB; reloj UTC; append-only por trigger SQLite; mutación más evento en una UoW; y la ausencia de UPDATE/DELETE en el futuro repositorio.

**No se puede cerrar sin Auth Local:** tabla/identidad de usuario, FK compuesta actor-owner, `local_session_id`, lifecycle/revocación de sesión, obligatoriedad de correlación para USER, y el mecanismo DB-authoritative que construye `AuditActor` sin aceptar actor desde el cliente. El contrato Java `token_jti` queda deliberadamente fuera: era correlación de un JWT y no prueba que exista un equivalente Local.

**No se puede adoptar sin aceptación normativa:** nombre final singular/plural, representación UUID/timestamp/JSON, longitudes físicas de action/entity/reason, catálogo de entidades/acciones, índices de lectura Local, trigger exacto, política de retención y DDL aditivo. D01–D07 y ADR-0021/CC-0021 no sustituyen esta aceptación.

## Constraints, aislamiento e índices requeridos

La ficha aceptada debe especificar, como mínimo:

1. `CHECK` de actor:
   `USER => actor_user_id IS NOT NULL`; `SYSTEM => actor_user_id IS NULL`.
   Las reglas de `local_session_id` no se presuponen hasta aprobar la sesión
   Local; no se copia el `jti` JWT de Java.
2. FK compuestas que impidan Branch y actor de otra Organization. No sustituir
   scope por JSON, código de aplicación o IDs globales sin owner.
3. `CHECK` de action/entity_type no vacíos, límites explícitos para texto y
   metadata de tamaño finito. La aplicación validará metadata de forma
   recursiva, restringirá el árbol JSON y rechazará claves sensibles antes de
   insertarla.
4. Append-only por barrera DB: trigger o mecanismo SQLite equivalente que rechace
   `UPDATE` y `DELETE`; repositorio sin operaciones de mutación. Debe probarse
   contra SQLite real.
5. Índices guiados por las lecturas futuras: `(organization_id, occurred_at DESC,
   id DESC)`, y los compuestos por branch, actor y entidad solo después de
   confirmar su contrato de consulta.
6. Mutación de negocio + `audit_event` en la misma UoW SQLx y en un único commit.
   Un fallo de auditoría o commit revierte/no declara éxito de la mutación.

## Actor, sesión y autorización Local

La implementación Java obtiene `USER` de un access-JWT y exige su `jti`; también
permite `SYSTEM` para bootstrap/mantenimiento sin usuario técnico falso. Rust
Local no dispone aún de sesiones ni RBAC productivos, por lo que la ficha no
inventa un JTI, un token ni una relación de sesión.

Antes de habilitar un writer productivo debe decidirse cómo se une un principal
Local autenticado a `pos_user`, `local_session`, Organization y Branch, y cómo se
verifica esa decisión sin una caché de autorización obsoleta. Un principal
inatribuible debe fallar cerrado; no puede degradarse silenciosamente a `SYSTEM`.

## Plan de aceptación e implementación posterior

1. Titular normativo aprueba nombre, columnas, FKs, constraints, actor/sesión,
   índices, append-only y retención en F1–10..13/F4 correspondientes.
2. Se añade una migración SQLx Local **aditiva** aprobada; no se edita `0001`.
3. Se implementan value objects de metadata/actor, writer append-only y pruebas
   SQLite de UPDATE/DELETE rechazados, ownership compuesto, tamaño/secretos y
   rollback transaccional.
4. Se integra RBAC DB-authoritative y la sesión Local; solo entonces se elimina
   el uso de fixtures para casos de aplicación.

Hasta completar estos pasos, `SqliteOrganizationApplication` y
`SqliteBranchApplication` solo pueden usar writers de prueba aislados y siguen
siendo experimentales.
