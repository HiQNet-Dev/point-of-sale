# Modelo Organization — Branch (implementado)

Estado: **implementado** (migración `V2__create_organizations_and_branches.sql`,
módulos `organization` y `branch`). Versión `0.3.0`.

## Entidades

### organizations

| Columna | Tipo | Reglas |
| --- | --- | --- |
| `id` | `UUID` PK | Generado en aplicación (`UUID.randomUUID()`); nunca secuencial |
| `name` | `VARCHAR(120)` NOT NULL | Trim; no vacío (CHECK) |
| `code` | `VARCHAR(64)` NOT NULL UNIQUE | Slug `^[a-z0-9]+(-[a-z0-9]+)*$` (CHECK); **único global**; inmutable |
| `timezone` | `VARCHAR(64)` NOT NULL | Timezone IANA válido (validado con `ZoneId`) |
| `currency` | `VARCHAR(3)` NOT NULL | ISO 4217 (validado con Java `Currency`); CHECK longitud 3; mayúsculas |
| `locale` | `VARCHAR(20)` NOT NULL | BCP 47 (validado con `Locale.forLanguageTag` roundtrip) |
| `status` | `VARCHAR(16)` NOT NULL | CHECK `IN ('ACTIVE','INACTIVE')`; default `ACTIVE` |
| `inventory_negative_stock_policy` | `VARCHAR(16)` NOT NULL | CHECK `IN ('STRICT','ALLOW_NEGATIVE')`; default `STRICT` (V20); configuración organization-scoped, aún no consumida por el checkout |
| `version` | `BIGINT` NOT NULL | Optimistic locking (`@Version`); default 0 |
| `created_at` / `updated_at` | `TIMESTAMPTZ` NOT NULL | UTC (`Instant`), set por la entidad |

### branches

| Columna | Tipo | Reglas |
| --- | --- | --- |
| `id` | `UUID` PK | Generado en aplicación |
| `organization_id` | `UUID` NOT NULL | FK → `organizations(id)`; **toda operación lo exige** |
| `name` | `VARCHAR(120)` NOT NULL | Trim; no vacío (CHECK) |
| `code` | `VARCHAR(64)` NOT NULL | Slug (CHECK); **único dentro de la organización** (`UNIQUE (organization_id, code)`); inmutable |
| `timezone` | `VARCHAR(64)` NULL | IANA si existe; NULL = heredar de organización |
| `status` | `VARCHAR(16)` NOT NULL | CHECK `IN ('ACTIVE','INACTIVE')`; default `ACTIVE` |
| `version` | `BIGINT` NOT NULL | Optimistic locking |
| `created_at` / `updated_at` | `TIMESTAMPTZ` NOT NULL | UTC (`Instant`) |

Índice `idx_branches_organization_id` para listar por organización.

## Ownership y timezone

```text
Organization 1 ── N Branch   (la branch pertenece exactamente a una organización)
```

- `branch.timezone == NULL` → se usa el timezone de su organización
  (`Branch.effectiveTimezone()`).
- No se duplica configuración: la organizacion es la raíz de configuración;
  la branch solo sobreescribe lo que realmente difiere.

## Uniqueness

- `organizations.code`: **globalmente único** (decisión: cada instalación es
  normalmente un cliente/empresa; simplifica referencias técnicas).
- `branches.code`: único por organización (`UNIQUE(organization_id, code)`);
  el mismo code puede existir en organizaciones distintas.

## Invariantes implementadas

- name obligatorio + longitud ≤ 120; code slug obligatorio (normalizado a
  minúsculas + trim en el servicio, revalidado por la entidad).
- name se normaliza con `trim()` en la capa de dominio (create y update):
  nunca se persisten espacios externos; los espacios internos se conservan.
- timezone IANA real; currency ISO 4217 real (sin lista manual: `java.util.Currency`);
  locale BCP 47 reconstruible.
- status solo `ACTIVE`/`INACTIVE`; **sin hard delete** (la baja es status).
- `inventory_negative_stock_policy` (V20) solo `STRICT`/`ALLOW_NEGATIVE`,
  default `STRICT`. Política comercial de la organización; en C3a es
  configuración únicamente (el checkout aún no la consume, C3b pendiente) y no
  afecta ajustes manuales ni apertura de stock.
- version `BIGINT` con optimistic locking vía `@Version`; conflictos →
  `409 CONCURRENT_MODIFICATION`.

## Aislamiento (regla de oro)

`branch` nunca se resuelve por id global en operaciones API:
`findByIdAndOrganizationId`. Solicitar una branch de otra organización
devuelve 404. Véase `OrganizationIsolationTest` (patrón a replicar en módulos
futuros).

## Migración

- `V1__init_app_metadata.sql` intacta (no modificada).
- `V2__create_organizations_and_branches.sql` crea tablas, PK, FK, UNIQUE,
  CHECK y el índice. `ddl-auto=validate`: Hibernate nunca crea schema;
  Flyway es la única fuente.

## Decisiones pendientes

- `branch.timezone` en PATCH usa semántica tri-estado implementada
  (`PatchField` en `shared.api`): omitir = sin cambios; valor = override;
  `null` = limpiar override y heredar de la organización.
- Normalización de `code`: la API exige un slug ya formado
  (Bean Validation); el servicio además normaliza trim+lowercase para
  llamadas internas.
- No multi-tenancy compartido: `organization_id` escala datos dentro de una
  instalación; el despliegue sigue siendo por cliente
  (`Cliente A → DB A → Organization A`). Sin tenant resolver, RLS, subdomains
  ni control plane (ver ADR-0003).
- Reportes/auditoría futuros usarán `created_at`/`updated_at` UTC; la
  conversión a timezone de negocio se decide en la capa de presentación.