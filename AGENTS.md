# AGENTS.md — Reglas del repositorio HiQNet POS para agentes

Reglas repo-local para agentes automatizados que trabajan en este repositorio.
Complementan el flujo humano en `docs/05-development/git-workflow.md` y
`CONTRIBUTING.md`.

## Branching

- `main` es la rama estable/release: no se desarrolla directamente en `main`.
- `dev` es la rama permanente de integración/preproducción de la próxima release.
  Las features normales nacen de `dev` y se integran en `dev`.
- Ramas short-lived con prefijo de enfoque (`feat/backend/*`, `feat/frontend/*`,
  `feat/fullstack/*`, `fix/*`, `test/*`, `chore/*`, `docs/*`, `refactor/*`) creadas
  desde `origin/dev`; PR hacia `dev`.
- Release: `dev` → `main` solo tras la Release Audit y con CI verde.
- Hotfix: `hotfix/*` nace de `main`, vuelve a `main` y luego se sincroniza a `dev`.
- No crear ramas principales paralelas (`backend`, `frontend`, `develop`, `docker`,
  `integration`).

## Worktrees

- Trabaja directamente en el checkout principal con `git switch` y ramas
  short-lived.
- No uses worktrees por defecto. Solo crea uno cuando necesites mantener dos
  checkouts simultáneos y documenta la razón en el PR.
- Nunca commitees `.worktrees/` (es local y está en `.gitignore`).

## Pull requests

- Un PR, un propósito coherente (no mezclar Docker, features de UI, migraciones de
  schema y limpieza aleatoria).
- Los PR apuntan a `dev`; los `hotfix/*` apuntan a `main`.
- No commitear ni hacer push directo a `main` o `dev` (salvo tareas explícitas de
  release o sincronización controlada), ni force-push, ni reescribir historia
  publicada.
- Nunca commitear secretos ni `.env` reales.

## Contrato de API y documentación

- La especificación OpenAPI (`/v3/api-docs`; Swagger UI solo en `dev` con
  `app.security.permit-opendoc=true`) es la **fuente de verdad** del contrato
  HTTP.
- Todo cambio de contrato de API (endpoint, request, response, código de estado,
  permiso o código de error estable) debe actualizar OpenAPI **en el mismo
  cambio**: anotaciones (`@Operation`, `@Schema`, `@ApiResponse`) y los
  documentos de `docs/03-api` que apliquen.
- La forma del error (`ApiError`) se genera una sola vez en `OpenApiConfig`
  (`apiErrorSchemaCustomizer`); no dupliques el schema a mano.
- Un PR que cambia el contrato sin actualizar la especificación y su
  documentación es un cambio incompleto.

## Arquitectura Rust (obligatoria)

El backend Rust es un **monolito modular hexagonal**. Ver
`docs/01-architecture/rust-hexagonal-architecture.md`.

### Capas y dependencias

```text
pos-core <- pos-application <- pos-storage-sqlite <- pos-local
```

- **Domain (`pos-core`)**: agregados, value objects, invariantes y políticas
  puras. Prohibido SQLx, pools, HTTP, Tauri, configuración de infraestructura o
  cualquier dependencia de `pos-application`.
- **Application (`pos-application`)**: casos de uso, autorización contextual,
  puertos y construcción de eventos auditables. No conoce SQL, nombres de tabla
  ni SQLx.
- **Infrastructure (`pos-storage-sqlite`)**: conexiones, transacciones,
  repositorios, SQL, mappers y traducción de errores. No define reglas de
  negocio ni contratos de caso de uso.
- **Entrypoints (`apps/pos-local`)**: configuración, composición, migraciones
  autorizadas y ciclo de vida. Sin reglas de catálogo, ventas, inventario o
  autenticación, y sin SQL.

Los puertos se declaran en la capa que los consume. No crear traits, `dyn` ni
abstracciones sin un consumidor real. Los guard tests de
`crates/pos-application/src/architecture_tests.rs` deben seguir pasando.

### Transacciones y seguridad

- Toda mutación auditable comparte **una sola** `SqliteUnitOfWork`
  (`BEGIN IMMEDIATE`): mutación + auditoría + `commit` en la misma conexión.
- Prohibido abrir pools o transacciones adicionales dentro de una UoW, o
  confirmar mutaciones antes de auditar.
- Se conservan CAS, aislamiento organización/sucursal, índice parcial de turno
  abierto, serialización venta↔cierre y ledger append-only.
- La autorización es DB-authoritative; el actor se deriva siempre del
  `VerifiedPrincipal`, nunca de la entrada del cliente.

### Cambios y calidad

- Antes de una funcionalidad nueva, identificar módulo → dominio → caso de uso →
  puertos → adaptador → entrypoint → pruebas.
- No implementar lógica de negocio en handlers, repositorios SQLx ni `main.rs`.
- Archivos: >600 líneas exigen justificación; >1.000 priorizan extracción por
  responsabilidad. Nada de `utils.rs` misceláneos.
- Rustdoc y comentarios en español. Pruebas por feature, con SQLite real en
  infraestructura.
- No modificar contratos de negocio, DDL candidato, Flyway, gates de CI ni la
  versión del producto dentro de una refactorización.
- Registrar en un ADR cualquier decisión arquitectónica significativa.
