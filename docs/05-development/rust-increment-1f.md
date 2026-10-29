# Incremento 1F — identidad Local y cierre contractual de auditoría

**Estado: PARCIAL EXPERIMENTAL.** Se implementa dominio puro de identidad y
sesión, puertos de principal confiable y evidencia SQLite con fixtures. No se
habilita autenticación, RBAC, bootstrap, auditoría persistente ni DDL Local de
producto.

## Código materializado

`pos-core::identity` incorpora `PosUser`, `LocalCredential`, `LocalSession`,
`NormalizedEmail`, `SessionHandleHash`, `PermissionCode` y
`LocalPrincipalService`.

- User mantiene UUID y Organization inmutables, email normalizado como Java,
  estado ACTIVE/INACTIVE y patch CAS atómico.
- Credential está separada de User; almacena solo una referencia opaca redactada,
  tipo BCrypt legado o Argon2id candidato, policy, estado y revisión. No recibe ni
  expone una contraseña cruda.
- Session conserva UUID, user/organization owner, hash SHA-256 de handle opaco,
  creación, expiración, revocación y CAS. Su estado efectivo es ACTIVE, REVOKED
  o EXPIRED y toda validación de owner/sesión falla cerrada.
- El principal se construye únicamente tras consultar el repositorio de sesión,
  user vigente y `PermissionAuthority`. Sus campos son privados y solo él puede
  derivar `AuditActor` USER. `AuditActor` ya no tiene variantes ni constructores
  públicos; SYSTEM es una capacidad interna con procedencia cerrada.

La verificación criptográfica no se duplicó: el Migration Lab conserva vectores
sintéticos F4–19 para BCrypt/Argon2id. Sus parámetros no son una policy comercial
aprobada y no se aplican a datos reales.

## Reconciliación de `audit_event`

`AuditEventRecord` usa ahora los nombres canónicos candidatos de F1–10 §17.2:

| Antes 1E | 1F / candidato canónico | Significado |
| --- | --- | --- |
| `local_session_id` ausente | `session_id` | referencia a sesión Local, no JWT `jti` |
| `metadata` | `metadata_json` | JSON canónico validado ≤16 KiB |
| ausente | `event_version` | versión del contrato, `INTEGER NOT NULL`, inicial `1` |

`AuditEventVersion::INITIAL` vale 1 y prohíbe cero. No representa CAS ni permite
reinterpretar eventos ya persistidos; una evolución futura debe introducir una
versión compatible. La ficha física se actualizó sin crear DDL.

## Frontera de atribución

`AuditActorResolver`, publicado en 1E, se retiró en 1F. Devolvía un `AuditActor`
a un adaptador externo, lo que permitía fabricar atribución sin verificar sesión,
usuario ni permisos. El sustituto es `identity::LocalPrincipalService`:

```text
handle opaco -> adaptador criptográfico -> hash
  -> LocalSession vigente + PosUser de la misma Organization
  -> PermissionAuthority (DB vigente)
  -> VerifiedPrincipal (campos privados)
  -> audit_actor() / permits_scope()
```

`VerifiedPrincipal` no se puede construir fuera de `pos-core`: no expone
constructor ni campos mutables. `permits_scope` rechaza cualquier `AuditScope`
de otra Organization antes de que el caso de uso escriba o audite. `SYSTEM` sigue
siendo una capacidad `pub(crate)` con procedencia cerrada y sin consumidor
productivo.

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
- BCrypt legado solo es verificable/migrable tras login válido y con benchmark,
  parámetros e importación aprobados; no hay rehash productivo en este incremento.
- La normalización de metadata continúa sin detección de homoglifos Unicode,
  límite ya documentado en 1E.
- PostgreSQL 17 Migration Lab requiere Linux/WSL2; esta evidencia SQLite se
  ejecuta en Windows y no aprueba gates normativos.
