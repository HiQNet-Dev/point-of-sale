# Incremento 1H.1 — CAS obligatorio, sección crítica mínima y paquete D-AUTH

**Estado: LABORATORIO ENDURECIDO.** Se corrigen defectos de persistencia y
concurrencia detectados en 1H. No hay DDL productivo, login, sesiones emitidas ni
endpoints.

## 1. CAS obligatorio

En 1H `verify_and_upgrade` ignoraba el `CasOutcome` de `persist`. Ahora:

- `Updated` es la única salida que permite anunciar la transición.
- `Conflict` y `Missing` se colapsan en `IdentityError::ConcurrentModification`,
  sin revelar identidad ni existencia de otros tenants.
- El resultado `AppliedCredential { transition, persisted }` distingue lo que
  realmente se persistió; `rehash_performed = true` solo aparece tras `Updated`.
- El contador de fallos, el bloqueo y `RESET_REQUIRED` solo se anuncian cuando el
  CAS confirmó la fila.
- La auditoría se escribe únicamente cuando `persisted` es verdadero y comparte el
  commit; si falla, se revierte todo.

Ya no existe una ruta que llame a persistencia sin evaluar el resultado: `persist`
es privada y su único consumidor es `apply`.

## 2. Sección crítica mínima

Flujo nuevo:

```text
pool.acquire -> snapshot de credencial (lectura corta)
  -> spawn_blocking: BCrypt/Argon2id + hash de rehash
  -> BEGIN IMMEDIATE
       relectura de usuario y credencial
       validación de estado/modo
       CAS sobre credential_version
       auditoría si hubo persistencia
     COMMIT
```

Decisiones:

- La criptografía nunca se ejecuta con un lock de escritura retenido.
- El material no se reutiliza si la versión cambió: una verificación de snapshot
  no autoriza un upgrade sobre material distinto.
- Los intentos fallidos se aplican dentro de la transacción con relectura, de modo
  que dos fallos concurrentes no pierden incrementos.
- Un usuario desactivado entre snapshot y commit se rechaza sin mutación.
- La cancelación de la task antes del `BEGIN` no deja estado; después, el RAII de
  SQLx revierte.

## 3. Medición observada (Windows, x86_64, 12 CPU)

Medición del test `transaction_scope_excludes_crypto_duration` con
`lab-argon2-v1` (19456 KiB, t=2, p=1):

| Componente | Tiempo |
| --- | ---: |
| Preparación criptográfica (`prepare`, fuera de la transacción) | 392 ms |
| Transacción (`BEGIN IMMEDIATE` + apply + commit) | 2 ms |

En 1H ambos componentes ocurrían dentro de la transacción; la reserva de escritura
pasaba de ~2 ms a ~394 ms por intento. No se midió contención multihilo ni
hardware objetivo: el dato es local y no normativo.

## 4. Pruebas de concurrencia y seguridad

| Prueba | Resultado |
| --- | --- |
| `simultaneous_wrong_attempts_do_not_lose_updates` | dos fallos → contador 2, versión 3, dos auditorías |
| `simultaneous_upgrades_leave_one_winner_and_one_conflict` | un upgrade, un `ConcurrentModification`, versión final 2 |
| `concurrent_upgrade_cannot_reuse_a_stale_crypto_decision` | decisión previa rechazada tras cambio de material |
| `independent_writer_proceeds_while_argon2_verifies` | otro pool escribe mientras Argon2 corre |
| `cancelled_task_leaves_credentials_untouched_and_pool_usable` | abort sin mutación y pool reutilizable |
| `deactivated_user_between_snapshot_and_commit_is_rejected` | sin mutación |
| `audit_failure_rolls_back_the_credential_upgrade` | rollback completo |
| `disabled_and_locked_credentials_reject_before_expensive_verification` | sin verificación costosa |
| `wrong_password_locks_after_policy_and_never_upgrades` | umbral 3, ventana 60 s, test 4 |
| `corrupt_hash_requires_reset_without_fallback_or_further_mutation` | un solo cambio persistido y auditado |
| `session_handle_stores_only_its_hash` | nunca se persiste el handle |

Las pruebas de RBAC, recuperación, memberships, RAII, `BEGIN IMMEDIATE` y
auditoría de 1D–1G siguen ejecutándose.

## 5. Frontera de secretos

Revisado `pos-credentials` y los tipos internos:

- `SecretHash` y `SessionHandle` tienen `Debug` redactado; el hash expuesto solo
  por `expose_to_verifier`/`hash`.
- La contraseña solo se copia a `String` para `spawn_blocking`; no se registra ni
  se incluye en errores.
- `CredentialError` no contiene entrada ni hash; los errores de `bcrypt` se
  colapsan.
- Límites: 72 bytes BCrypt, 1024 Argon2id, 256 PHC, `m/t/p` exactos de política.
- No hay fallback a algoritmos débiles ni parámetros por defecto.
- Las políticas siguen marcadas como experimentales y no son configuración
  comercial.

## 6. Paquete D-AUTH definitivo

| ID | Contrato vigente | Propuesta | Evidencia | Divergencia | Riesgo | Aceptación | ADR | Recomendación |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| D-AUTH-01 | F1–10 §16.4/16.5/16.45 | Crear `pos_user`, `local_credential`, `local_session` con nombres canónicos; `display_name` incluido | Java V3/V4; 1F | `display_name` no existe en Java V3 | Bajo | Round-trip de todas las columnas | No | Aprobar con `display_name` confirmado |
| D-AUTH-02 | §16.5 `ACTIVE/PASSWORD_RESET_REQUIRED` | CHECK físico con esos dos valores; `Disable` se modela con `pos_user.status` | Java `users.status`; 1H | El laboratorio usa `RESET_REQUIRED` y `DISABLED` (fixture) | Medio | `CHECK` rechaza valores fuera del catálogo | Sí, si se añade `DISABLED` | Corregir el laboratorio a `PASSWORD_RESET_REQUIRED`; no ampliar el catálogo |
| D-AUTH-03 | §16.45 incluye columna `state` | Añadir `state IN ('ACTIVE','REVOKED','EXPIRED')` y escribirla en cada transición; el dominio puede derivarla | F1–10; Java `refresh_sessions` | 1G propuso derivarlo sin columna | Medio | Coherencia `state`↔`revoked_at`/`expires_at` | Sí | Corregir: la columna es física |
| D-AUTH-04 | §16.45 no incluye `organization_id` | Sin columna; ownership por FK a `pos_user` | F1–10 | Ninguna | Bajo | FK compuesta y consulta con `JOIN` | No | Aprobar |
| D-AUTH-05 | `session_version` inicial 1 | `credential_version` inicial 1 y ambos como CAS | 1F/1G/1H | `credential_version` no fija default canónico | Bajo | CAS concurrente probado | No | Aprobar |
| D-AUTH-06 | 59 permisos sembrados (V5–V25) | Replicar catálogo exacto y roles; rol administrador ordinario | `rust-parity-baseline.md` §4.3; Java V5/V7 | Local no tiene catálogo | Medio | Conteo y códigos idénticos | No | Aprobar; sembrar por migración |
| D-AUTH-07 | §17.2 `session_id` nullable | FK compuesta opcional; USER exige `actor_user_id`, SYSTEM lo prohíbe; SYSTEM solo por camino interno | Java V7; 1E | `token_jti` no se copia | Medio | CHECK de actor y FK same-org | No | Aprobar |
| D-AUTH-08 | Append-only | Trigger `BEFORE UPDATE/DELETE`; sin pretensión criptográfica | Java V7 | Ninguna | Bajo | UPDATE/DELETE rechazados | No | Aprobar con alcance declarado |
| D-AUTH-09 | F4–19 pendiente | Mantener bloqueada la elección comercial de Argon2id | Benchmark local 2026 | Laboratorio usa `lab-argon2-v1` | Alto | Benchmark en hardware objetivo + interoperabilidad | Sí | Bloquear |
| D-AUTH-10 | Java V8 | `bootstrap_record` singleton PENDING/COMPLETED; secreto en despliegue | Java V8 | Local sin implementación | Medio | Guard pre-body y one-shot | No | Aprobar contrato; implementar en 1I |

Ninguna fila está aprobada: requieren acto explícito del titular.

## 7. Orden físico y comprobaciones

| Orden | Migración propuesta | Depende de |
| --- | --- | --- |
| 1 | `0002_user_credential_session.sql` | `0001` |
| 2 | `0003_rbac_catalog.sql` | `0002` |
| 3 | `0004_branch_membership.sql` | `0001`, `0002`, `0003` |
| 4 | `0005_audit_event.sql` | `0001`–`0003` |
| 5 | `0006_bootstrap_record.sql` | `0001`, `0002`, `0003` |

Controles previstos para 1I: `STRICT`, CHECK de catálogos, `UNIQUE(id,
organization_id)`, FKs compuestas, índices de consulta, trigger append-only y
pruebas de CAS/rollback sobre el esquema real. No se crea ninguna migración hasta
la aceptación de D-AUTH-01..10.

## 8. Límites

- Sin login, sesiones emitidas, refresh, revocación de familias ni bootstrap.
- Sin RBAC productivo ni auditoría persistente.
- Sin endpoints Axum/OpenAPI.
- Migration Lab PostgreSQL 17 no ejecutado en este entorno.
- `PAR-08` sigue siendo el único gate histórico aprobado.
