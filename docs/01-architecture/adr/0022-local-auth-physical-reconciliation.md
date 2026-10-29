# ADR-0022 (propuesto) — Reconciliación física de identidad, sesión y auditoría Local

**Estado: PROPUESTO. No aprobado. No altera el Design Freeze.**
**Fecha: 2026-10-29. Incremento: 1H.2.**

## Contexto

Los incrementos 1E–1H construyeron dominio y adaptadores de laboratorio para
identidad, sesión, permisos, credenciales y auditoría. Tres decisiones de esos
incrementos se apartan del diccionario físico canónico y deben resolverse antes de
crear las migraciones `0002`–`0006`.

## Decisiones propuestas

### 1. `local_credential.state` (D-AUTH-02)

- **Contrato vigente:** F1–10 §16.5 define `ACTIVE` y `PASSWORD_RESET_REQUIRED`.
- **Uso actual:** el fixture de laboratorio emplea `ACTIVE`, `RESET_REQUIRED` y
  `DISABLED`; el dominio `CredentialState` mantiene las tres variantes.
- **Propuesta:** la columna física acepta solo `ACTIVE` y
  `PASSWORD_RESET_REQUIRED`. La desactivación se modela con `pos_user.status`.
- **Divergencia:** el fixture no coincide con el catálogo físico.
- **Riesgo:** si se añade `DISABLED` sin contrato, aparecen dos fuentes de verdad
  para la desactivación.
- **Pruebas de aceptación:** `CHECK` rechaza cualquier otro valor; desactivar un
  usuario bloquea la verificación sin tocar la credencial.
- **Recomendación:** aprobar; alinear el fixture en 1I.

### 2. `local_session.state` (D-AUTH-03)

- **Contrato vigente:** F1–10 §16.45 incluye una columna física `state`.
- **Uso actual:** 1G propuso derivar el estado solo de `expires_at`/`revoked_at`.
- **Propuesta:** añadir la columna `state IN ('ACTIVE','REVOKED','EXPIRED')`,
  escrita en cada transición, con `CHECK` de coherencia con `revoked_at`.
- **Divergencia:** la propuesta 1G omitía una columna especificada.
- **Riesgo:** doble fuente si la columna no se actualiza atómicamente con los
  timestamps; mitigado por los `CHECK` y por escribir ambos en la misma sentencia.
- **Pruebas de aceptación:** revocar y expirar producen `state` coherente; una
  fila `ACTIVE` con `revoked_at` es rechazada.
- **Recomendación:** aprobar la corrección; mantener la derivación en el dominio.

### 3. `audit_event` (D-AUTH-07/08)

- Se conservan `session_id`, `metadata_json` y `event_version`; no se introduce
  `local_session_id` ni se copia `token_jti`.
- `session_id` es nullable y usa FK compuesta `(session_id, actor_user_id)` hacia
  `local_session(id, user_id)`.
- `USER` exige `actor_user_id`; `SYSTEM` lo prohíbe y solo procede de caminos
  internos explícitos.
- Append-only por trigger, sin pretensión criptográfica.

## Consecuencias

- 1I puede crear las migraciones `0002`–`0006` sin rehacer el dominio; los
  adaptadores `*_lab` se sustituyen por adaptadores sobre tablas canónicas.
- El fixture de credenciales debe cambiar `DISABLED`/`RESET_REQUIRED` por el
  catálogo físico.
- Argon2id comercial sigue bloqueado (D-AUTH-09) y no depende de este ADR.

## Aceptación requerida

Acto explícito del titular sobre D-AUTH-01..08 y D-AUTH-10. D-AUTH-09 permanece
bloqueada. Sin esa aceptación este ADR no habilita ninguna migración productiva.
