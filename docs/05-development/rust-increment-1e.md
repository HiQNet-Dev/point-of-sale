# Incremento 1E — foundation de auditoría Local

**Estado: PARCIAL EXPERIMENTAL.** Esta entrega añade dominio Rust y evidencia
SQLite de laboratorio para auditoría. No habilita una tabla Local, writer
persistente de producto, Auth/RBAC DB-authoritative, sesiones, HTTP u OpenAPI.

## Alcance implementado

`pos-core::audit` contiene tipos puros y documentados en español:

- `AuditEventId`, `AuditEventRecord`, `AuditScope` y `AuditOccurredAt`;
- `AuditActor::{User,System}` y `SystemProvenance`, sin usuarios técnicos
  artificiales;
- `AuditAction`, `AuditEntityType` y `AuditReason` con la validación actual de
  Java preservada (no vacío tras `String.trim()` para action/entity, sin imponer
  un catálogo Rust no aprobado);
- `AuditMetadata`: objeto JSON, inspección recursiva de objetos/arreglos,
  profundidad máxima técnica 32, serialización canónica, 16 KiB UTF-8 máximos y
  rechazo sin truncamiento de claves sensibles normalizadas.

La política bloquea `password`, `token`, `authorization`, `jwt`, `secret`,
`privatekey`, `credential` y `apikey`, incluso si aparecen en niveles anidados
o se separan con puntuación. Los valores son `serde_json::Value`; por tanto, no
se acepta un POJO arbitrario ni números no JSON. La metadata validada conserva
un árbol propio y el JSON canónico es exactamente el texto que un writer futuro
debe persistir.

## Frontera de identidad futura

`AuditActorResolver` recibe solo un `AuditScope` de la operación. Un adaptador
futuro de Auth/RBAC debe derivar el usuario, organización, sucursal y permisos de
un principal/sesión verificados contra la base de datos, y debe fallar cerrado
con `AuditAttributionError` si no puede atribuirlo. El cliente no proporciona
`AuditActor`.

No se ha agregado `local_session_id`: su ownership, FK, lifecycle, revocación y
obligatoriedad no están aprobados. El `token_jti` JWT de Java tampoco se copia a
Local. `SYSTEM` se reserva a caminos internos explícitos (`SecureBootstrap`,
mantenimiento controlado o flujo interno); un principal autenticado no
atribuible no puede degradarse a SYSTEM.

> **Actualización 1F.** `AuditActorResolver` se retiró: un trait público que
devolvía `AuditActor` habilitaba atribución no verificada desde adaptadores
externos. La sustituye `pos-core::identity::LocalPrincipalService`, que entrega
un `VerifiedPrincipal` de campos privados desde el que se deriva el actor. El
campo canónico es `session_id`; ver [`rust-increment-1f.md`](rust-increment-1f.md).

## Contrato transaccional y SQLite

La composición existente se conserva sin duplicarla:

```text
BEGIN IMMEDIATE (sqlx::Transaction RAII)
  -> mutación/CAS
  -> writer de auditoría transaccional
  -> COMMIT único
```

`SqliteUnitOfWork::begin` es el único punto que usa el constructor público pero
`#[doc(hidden)]` de SQLx 0.9.0:
`Transaction::begin(connection, Some(SqlStr::from_static("BEGIN IMMEDIATE")))`.
No se emite un segundo `BEGIN` ni se implementa rollback async en `Drop`. Todo
cambio de la versión bloqueada de SQLx exige revisar esa API y volver a ejecutar
las pruebas de lock entre pools, drop, cancelación, CAS y fallo de commit antes
de modificar la estrategia.

Los tests crean `audit_event_fixture` exclusivamente durante su ejecución. La
fixture usa `STRICT`, `json_valid`, límite de bytes UTF-8, `CHECK` USER/SYSTEM,
FK compuesta branch-organization y triggers que abortan UPDATE/DELETE. Prueba:

- inserción de evento y mutación de Organization en una transacción;
- actor USER sin usuario rechazado;
- branch de otra organización rechazado;
- JSON inválido rechazado por SQLite;
- UPDATE y DELETE rechazados por trigger;
- rollback cuando falla el insert de auditoría;
- cancelación de task después de insertar negocio y evento, seguida por
  reacquisición acotada de la UoW, sin efectos persistidos.

La fixture no es una migración, no contiene usuario/sesión Local ni se conecta a
ninguna composición de producción.

## Paridad y diferencias conocidas

| Capa | Estado |
| --- | --- |
| Metadata segura, actor explícito y hechos puros | `DOMINIO_RUST_VERIFICADO` |
| Atomicidad Organization/Branch + fixture | `SQLITE_LAB_VERIFICADO` |
| Trigger append-only y scope de fixture | `SQLITE_LAB_VERIFICADO` |
| Writer `audit_event` persistente Local | Pendiente de DDL aceptado |
| Actor USER autenticado / sesión Local / RBAC | Pendiente |
| API de lectura, filtros, OpenAPI y React | Pendiente |
| Paridad integral Java/Rust | Pendiente |

Java V7 persiste `token_jti` para su JWT y aplica longitudes PostgreSQL
`VARCHAR(80/40/255)`. Rust no incorpora el JTI ni afirma aún la representación
física Local de action/entity/reason. Los constructores Rust conservan la
validación de aplicación Java para action/entity; las decisiones adicionales se
mantienen en la ficha física, no se presentan como paridad ya demostrada.

La normalización de claves sensibles conserva el enfoque Java: minúsculas y
eliminación de puntuación para comparar tokens ASCII. No implementa detección de
homoglifos Unicode; los productores de metadata deben usar claves técnicas ASCII
y cualquier ampliación de normalización requiere una decisión de contrato y
vectores diferenciales Java/Rust.

## Esquema y gates

No cambió Flyway V1–V25 ni `backend-rust/migrations/local/0001_organization_branch.sql`.
No se creó DDL Local de auditoría. `audit_event` sigue siendo una propuesta
revisable en `rust-local-audit-event-proposal.md`; DDL13/PAR/AS no se promocionan.
PAR-08 sigue siendo el único PASS histórico y los otros 29 casos normativos
continúan bloqueados/no ejecutados.

En Windows se ejecutan los checks y SQLite. PostgreSQL 17 del Migration Lab
requiere Linux/WSL2 según `backend-rust/README.md`; no se ejecutó en esta entrega
Windows y no se ha marcado ningún caso no ejecutado como aprobado.
