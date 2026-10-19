# PROJECT_MAP — HiQNet POS

Índice de contexto para agentes. **No es una copia del repo ni una bitácora**:
sólo apunta a las fuentes de verdad. Actualízalo únicamente cuando cambien de
forma estable la arquitectura, los comandos o las fuentes de verdad.

## Propósito y stack

POS modular para pequeños negocios de alimentos y retail ligero. **Modular
Monolith**: una API Spring Boot + una SPA React, sin microservicios.

| Capa | Stack | Raíz |
| --- | --- | --- |
| Backend | Java 21, Spring Boot 4.1.1, Maven Wrapper, Spring Security (JWT), Spring Data JPA, Flyway, PostgreSQL 17, Testcontainers | `backend/` |
| Frontend | React 19, TypeScript estricto, Vite, React Router 7, TanStack Query 5, MUI | `frontend/` |
| Docs | Documentación viva (producto, arquitectura/ADR, API, seguridad, dev, testing) | `docs/` |
| Deploy | Estrategia de despliegue (sólo documentación) | `deploy/` |

Versión actual: `VERSION` (hoy `0.4.0`, Purchasing).

## Rutas de entrada por tipo de tarea

| Tarea | Empieza por |
| --- | --- |
| Feature de backend | `backend/src/main/java/com/hiqnet/pos/<modulo>/` (`api`, `application`, `domain`, `infrastructure` opcional) |
| Contrato HTTP / endpoint | `backend/.../<modulo>/api/*Controller.java` + `docs/03-api/` |
| Migración de esquema | `backend/src/main/resources/db/migration/` (Flyway, sólo en orden) |
| Seguridad / RBAC / permisos | `docs/04-security/security-model.md`, `backend/.../rbac/`, `docs/03-api/organization-and-branch-context.md` |
| Feature de frontend | `frontend/src/features/<feature>/` (`api`, `model`, `components`, `pages`) |
| Routing / shell frontend | `frontend/src/app/router/index.tsx`, `frontend/src/app/providers/` |
| Cliente HTTP / sesión | `frontend/src/shared/api/client.ts`, `frontend/src/features/auth/` |
| Pruebas backend | `backend/src/test/java/com/hiqnet/pos/<modulo>/` (Testcontainers) |
| Pruebas frontend | `*.test.ts(x)` junto al código; E2E en `frontend/e2e/` |
| Entorno local / fixtures | `docs/05-development/local-development.md`, `docker-compose.yml`, `frontend/scripts/dev/` |
| CI | `.github/workflows/ci.yml`, `docs/05-development/ci.md` |

## Documentos y archivos canónicos

| Tema | Fuente |
| --- | --- |
| Reglas obligatorias del repo | `AGENTS.md` (incluye branching, worktrees, PRs, contrato de API) |
| Arquitectura y límites de módulos | `docs/01-architecture/architecture.md`, `docs/01-architecture/modules.md`, `docs/01-architecture/adr/` |
| Roadmap y estado del producto | `docs/00-product/roadmap.md`, `README.md` |
| Contrato de API (fuente de verdad) | OpenAPI en `/v3/api-docs` (dev) + `docs/03-api/README.md` y sus guías |
| Esquema de base de datos | `backend/src/main/resources/db/migration/` + `docs/02-database/` |
| Seguridad | `docs/04-security/security-model.md`, `docs/04-security/authentication.md`, `docs/04-security/secure-bootstrap.md` |
| Estilo de código | `docs/05-development/coding-guidelines.md` |
| Flujo Git / PRs / releases | `docs/05-development/git-workflow.md`, `CONTRIBUTING.md`, `docs/05-development/release-process.md` |
| Entorno local y fixtures | `docs/05-development/local-development.md` |
| E2E y auditoría UX | `docs/05-testing/pos-e2e.md`, `docs/05-testing/frontend-ux-audit.md` |
| Build/CI | `backend/pom.xml`, `frontend/package.json`, `.github/workflows/ci.yml` |

## Módulos backend implementados

`auth`, `organization`, `branch`, `users`, `rbac`, `membership`, `audit`,
`bootstrap`, `catalog`, `cashregister`, `cashshift`, `sales`, `payment`,
`inventory`, `purchasing`, `shared`. Paquete base:
`com.hiqnet.pos.<modulo>`; split `domain/application/infrastructure/api`
**opcional** (no crear paquetes vacíos).

## Features frontend actuales

`auth`, `admin-shell`, `pos-session`, `pos-sales`, `inventory-items`,
`inventory-stock`, `inventory-recipes`, `inventory-modifier-effects`,
`inventory-settings`. Rutas protegidas: `/`, `/pos`, `/admin/**`; login en
`/login`.

## Comandos de validación (comprobados)

```bash
# Backend: compila + pruebas (Testcontainers, PostgreSQL real; sin H2)
cd backend && ./mvnw clean verify          # Windows: backend\mvnw.cmd clean verify

# Frontend
cd frontend && pnpm install
pnpm lint && pnpm typecheck && pnpm test && pnpm build
pnpm typecheck:e2e
pnpm e2e                                   # Playwright (requiere stack E2E)
```

No ejecutes la suite completa para cambios que sólo son de configuración o
documentación; usa el check específico y justifícalo.

## Herramientas de símbolos disponibles

- Serena MCP (activo): `find_symbol`, `find_referencing_symbols`,
  `search_for_pattern`, `get_symbols_overview` — úsalo para localizar y leer
  sólo implementación, interfaces y pruebas relacionadas.
- LSP del IDE (Zed/OpenCode) para definiciones y referencias.

## Reglas críticas (consultar la fuente, no copiar aquí)

- Dinero con `BigDecimal`, operaciones monetarias transaccionales, sin hard
  delete financiero: `docs/05-development/coding-guidelines.md`.
- Esquema sólo con Flyway; `ddl-auto: validate`, nunca `update`.
- Validación y autorización server-side; el frontend es UX, no autoridad.
- Scoping: catálogo pertenece a la **Organization**; existencias a la **Branch**.
- Todo cambio de contrato HTTP actualiza OpenAPI y `docs/03-api` en el mismo
  cambio: `AGENTS.md`.
- Nunca tocar/commitear secretos, `.env` reales ni datos de producción.
