# Incremento 1F — identidad Local y cierre contractual de auditoría

**Estado: PARCIAL EXPERIMENTAL.** Se implementa dominio puro de identidad y
sesión, puertos de principal confiable y evidencia SQLite con fixtures. No se
habilita autenticación, RBAC, bootstrap, auditoría persistente ni DDL Local de
producto.

## Código materializado

`pos-core::identity` incorpora `PosUser`, `LocalCredential`, `LocalSession`,
`NormalizedEmail`, `SecretHash`, `SessionHash`, `PermissionCode`,
`PrincipalScope` y `LocalPrincipalService`.

- User mantiene UUID y Organization inmutables, email normalizado como Java,
  estado ACTIVE/INACTIVE y patch CAS atómico.
- Credential está separada de User y su material nunca se expone.
- Session conserva UUID, user owner, hash SHA-256 del handle opaco, creación,
  expiración, revocación y versión. Su estado efectivo es ACTIVE, REVOKED o
  EXPIRED y toda validación de owner/sesión falla cerrada.
- El principal se construye únicamente tras consultar el repositorio de sesión,
  user vigente y `PermissionAuthority`. Sus campos son privados y solo él puede
  derivar `AuditActor` USER.

La verificación criptográfica no se duplicó: el Migration Lab conserva vectores
sintéticos F4–19 para BCrypt/Argon2id. Sus parámetros no son una policy comercial
aprobada y no se aplican a datos reales.

## Correcciones de contrato aplicadas

### Credencial

F1–10 §16.5 separa tres conceptos que la versión anterior mezclaba:

| Antes | Ahora | Significado |
| --- | --- | --- |
| `CredentialKind::{LegacyBcrypt, Argon2id}` | `CredentialKind::{Password, Pin}` | finalidad (`credential_kind`) |
| — | `CredentialAlgorithm::{LegacyBcrypt, Argon2id}` | algoritmo (`algorithm`) |
| `CredentialReference` | `SecretHash` | `secret_hash`, redactado |
| — | `CredentialParameters` | `params_json` validado, ≤2048 bytes |
| `revision` | `credential_version` (inicia 1) | versión del material + CAS |
| — | `failed_login_attempts`, `locked_until` | bloqueo temporal Java |
| `CredentialState::Ready` | `CredentialState::Active` | estado declarado |

`record_failed_login` replica `User.recordFailedLogin`: una ventana de bloqueo
vencida reinicia el contador; al alcanzar el máximo se fija `locked_until`.
`record_successful_login` limpia contador y bloqueo. `rotate_material` sustituye
algoritmo, hash, parámetros y política, y sirve de base a BCrypt → Argon2id sin
implementar criptografía.

`Disabled` y `ResetRequired` se documentan como estados de dominio: Java modela
la desactivación en `users.status`, no en una credencial independiente, y el
catálogo físico de `local_credential.state` sigue pendiente de aprobación.

### Sesión

| Antes | Ahora | Motivo |
| --- | --- | --- |
| `handle_hash` / `SessionHandleHash` | `session_hash` / `SessionHash` | nombre canónico §16.45 |
| `revision` inicial `0` | `session_version` inicial `1` | §16.45 fija versión inicial 1 |
| — | `ensure_owned_by(&PosUser)` | ownership explícito antes del principal |

`organization_id` se conserva como ownership de agregado derivado de `pos_user`;
**no** se propone como columna física porque §16.45 no la incluye. El adaptador
debe obtenerla del usuario y verificar coincidencia.

### Autorización contextual

La versión anterior aceptaba `branch_id` para resolver permisos pero descartaba
el alcance, de modo que un permiso de la sucursal A podía reutilizarse en B.

Ahora `VerifiedPrincipal` conserva el `PrincipalScope` exacto y expone
`authorize(permission, scope)`. Scope distinto → `ScopeMismatch`; permiso ausente
→ `PermissionDenied`. Se retiraron `allows` y `permits_scope` para que no exista
una decisión de permiso sin contexto. El contrato de `PermissionAuthority`
especifica la unión organización + membership activa para scope de sucursal y el
rechazo de sucursales ajenas o inactivas.

`authenticate` es una decisión puntual: para mutaciones críticas los permisos
deben re-resolverse dentro de la misma UoW, sin caché como autoridad de
escritura. La implementación ejecutable de esa frontera corresponde al
incremento 1G.

## Reconciliación de `audit_event`

`AuditEventRecord` usa los nombres canónicos candidatos de F1–10 §17.2:

| Antes 1E | 1F / candidato canónico | Significado |
| --- | --- | --- |
| `local_session_id` ausente | `session_id` | referencia a sesión Local, no JWT `jti` |
| `metadata` | `metadata_json` | JSON canónico validado ≤16 KiB |
| ausente | `event_version` | versión del contrato, `INTEGER NOT NULL`, inicial `1` |

`AuditEventVersion::INITIAL` vale 1 y prohíbe cero. No representa CAS ni permite
reinterpretar eventos ya persistidos; una evolución futura debe introducir una
versión compatible. La ficha física se actualizó sin crear DDL.

## Evidencia SQLite fixture

Una fixture `STRICT` independiente verifica `pos_user_fixture` y
`local_session_fixture`: email único por Organization, mismo email permitido en
otra Organization, FK compuesta user/organization, rechazo cross-organization y
CAS de revocación con dos pools independientes. No es una migración y no se usa
en producción.

Las pruebas 1D/1E de `BEGIN IMMEDIATE`, RAII, cancelación, rollback, CAS,
append-only y auditoría fixture permanecen sin cambio.

## Bootstrap y esquema

Java V8 sigue siendo el antecedente funcional de bootstrap: singleton, instalación
prístina, provisioning de Organization/User/Role/grants, auditoría SYSTEM y un
commit. Rust 1F solo documenta `bootstrap_record` como dependencia física;
carece de secret guard pre-body, catálogo RBAC Local, writer y tablas aceptadas.

No cambian Flyway V1–V25 ni `migrations/local/0001_organization_branch.sql`.
La matriz de propuestas está en
[`rust-local-identity-proposal.md`](rust-local-identity-proposal.md). DDL13/PAR/AS
no se promocionan; PAR-08 continúa como único PASS histórico.

## Límites de seguridad

- No se importan sesiones Java ni se copian `token_jti`.
- No existe allow-all productivo ni permisos desde DTO/JWT.
- `Role`/`Membership` no están modelados como agregados: solo existe el puerto
  `PermissionAuthority`; el catálogo y las tablas siguen pendientes.
- BCrypt legado solo es verificable/migrable tras login válido y con benchmark,
  parámetros e importación aprobados; no hay rehash productivo en este incremento.
- La normalización de metadata continúa sin detección de homoglifos Unicode,
  límite ya documentado en 1E.
- PostgreSQL 17 Migration Lab requiere Linux/WSL2; esta evidencia SQLite se
  ejecuta en Windows y no aprueba gates normativos.
