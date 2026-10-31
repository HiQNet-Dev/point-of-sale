# Incremento 1H.2 — cierre de seguridad criptográfica y preparación de 1I

**Estado: LABORATORIO ENDURECIDO + PROPUESTAS FÍSICAS.** No hay DDL productivo,
login, sesiones emitidas ni endpoints. Todas las propuestas requieren aceptación
explícita.

## 1. P0 — Exposición de contraseñas por `Debug`

- `CredentialAttempt` ya no deriva `Debug`: usa una implementación manual que
  imprime `<redacted>` para `password`.
- `PreparedCredential` conserva su `Debug` redactado; el fingerprint de material
  no deriva `Debug` y nunca aparece en diagnósticos.
- `SecretHash`, `SessionHash` y `SessionHandle` mantienen `Debug` redactado;
  `CredentialParameters` solo expone parámetros no secretos ya validados.
- `CredentialError`/`IdentityError` no transportan entrada ni material.
- Prueba: `debug_output_never_contains_the_secret` verifica `CredentialAttempt`,
  `PreparedCredential` y `LocalCredential`.

## 2. P1 — Decisiones criptográficas obsoletas

`apply` compara ahora un **fingerprint de material** (`algorithm`, `policy_id`,
`params_json`, `secret_hash`) en lugar de exigir igualdad de versión:

| Cambio entre snapshot y apply | Comportamiento |
| --- | --- |
| Hash, algoritmo o parámetros distintos | `ConcurrentModification`, sin mutación |
| Solo versión por intentos fallidos | se aplica sobre la fila vigente (sin pérdida) |
| Estado pasa a `RESET_REQUIRED` | `ResetRequired` sin mutación |
| Estado pasa a deshabilitado / usuario inactivo | `Rejected` sin mutación |
| Upgrade concurrente ya confirmado | `ConcurrentModification` |

Esto elimina la carrera en la que un `Rejected` basado en un hash anterior
incrementaba el contador del material nuevo, sin impedir que dos contraseñas
incorrectas sobre el mismo material sumen ambos intentos.

Pruebas: `rejected_attempt_after_rotation_is_not_applied`,
`reset_and_disable_between_snapshot_and_apply_are_observed`,
`simultaneous_wrong_attempts_do_not_lose_updates`,
`concurrent_upgrade_cannot_reuse_a_stale_crypto_decision`.

## 3. P1 — Secretos temporales y saturación

- La contraseña se copia a `Zeroizing<String>` y se borra al salir del alcance.
  Solo existe una copia, creada en la frontera del adaptador.
- `VerificationGate` limita las verificaciones costosas con un `Semaphore`
  acotado; el permiso se mueve al trabajo bloqueante, de modo que cancelar el
  future que espera no libera el slot mientras el cálculo sigue vivo.
- Cancelar el future no detiene `spawn_blocking` ya iniciado: el resultado se
  descarta y no hay transacción abierta.
- Nunca se retiene un lock de SQLite esperando capacidad: el snapshot termina
  antes y la transacción empieza después de la criptografía.

Pruebas: `gate_limits_concurrent_verifications`,
`cancelled_while_waiting_for_capacity_leaves_state_consistent`,
`independent_writer_proceeds_while_argon2_verifies`,
`cancelled_task_leaves_credentials_untouched_and_pool_usable`.

## 4. Invariantes de Auth revisadas

| Regla | Estado en Rust (laboratorio) |
| --- | --- |
| Login correcto | `Verified`, limpia contador/bloqueo con CAS |
| Login fallido | incrementa contador, bloquea al umbral |
| Ventana de bloqueo | reinicia contador al vencer; bloqueo activo rechaza antes de verificar |
| Reset obligatorio | `PASSWORD_RESET_REQUIRED` lógico, sin fallback |
| Usuario inactivo | rechazo antes de verificar y revalidación dentro de la transacción |
| Sesión revocada | no construye principal (1G) |
| Credencial rotada | decisión previa inválida por fingerprint |
| CAS concurrente | un ganador; conflicto sin mutación |
| Auditoría | solo cambios persistidos, en el mismo commit |
| BCrypt legado | verificación y rehash, sin truncado |
| Upgrade oportunista | solo tras verificación correcta y CAS |

No se emite sesión ni se declara autenticación productiva.

## 5. Frontera de secretos reauditada

- `SecretHash`/`SessionHandle` con `Debug` redactado; sin `Serialize`.
- Handle de sesión de 32 bytes CSPRNG; solo se persiste SHA-256.
- Límites: BCrypt 72 bytes, Argon2id 1024 bytes, PHC 256, parámetros exactos.
- Rechazo de políticas desconocidas y de fallback a algoritmos débiles.
- Las políticas siguen marcadas como experimentales.

## 6. Propuestas físicas

- Paquete de migraciones propuesto en
  [`backend-rust/proposals/local-auth/`](../../backend-rust/proposals/local-auth/README.md),
  fuera del root de migraciones y no cargado por `pos-local`.
- ADR propuesto:
  [`ADR-0022`](../01-architecture/adr/0022-local-auth-physical-reconciliation.md),
  que documenta la corrección de `local_session.state` (D-AUTH-03) y el catálogo
  de `local_credential.state` (D-AUTH-02).

## 7. CI

El workflow existente (`.github/workflows/ci.yml`) ya ejecuta Rust en
`ubuntu-latest` con `cargo fmt`, `clippy`, `test`, `build`, gitleaks y el gate
DDL13. No se modificó ni se desactivó ningún control Java/frontend. No hay
ejecución de CI para esta rama en este entorno.

## 8. Límites

- Sin DDL productivo, login, sesiones emitidas, RBAC productivo ni auditoría
  persistente.
- Migration Lab PostgreSQL 17 no ejecutado aquí.
- `PAR-08` sigue siendo el único gate histórico aprobado.
