# HiQNet POS

Sistema punto de venta modular de [HiQNet](https://github.com/HiQNet-Dev), orientado
principalmente a **pequeños negocios de alimentos** (cafeterías, juguerías, restaurantes
pequeños) y **retail ligero**.

> **Estado:** HiQNet POS está en desarrollo activo y todavía **no es una release
> comercial estable**. La versión en desarrollo es `0.3.0-SNAPSHOT`
> (ver `VERSION`); el último milestone liberado es **`v0.2.0` (POS Core)**,
> release técnica privada. POS Core incluye catálogo operativo, cajas, turnos,
> ventas y checkout. Inventory v0.3 ya tiene catálogo y stock por sucursal con
> ledger, apertura y ajustes, recipes y efectos de modificadores, UI de
> administración y consumo de inventario en checkout; la UI de política de stock
> negativo y el E2E integrado de inventario siguen pendientes.

## Foundation implementada (v0.1.x)

La fundación actual ya es operativa como backend administrativo seguro:

- **Organization / Branch / Users** — administración completa scoped por organización.
- **Authentication** — login, refresh rotativo (30d, familia 30d), logout en
  familia, `logout-all`, `/me`; JWT identity-only, PostgreSQL autoritativa.
- **Roles / Permissions / RBAC** — catálogo de 19 permisos sembrado por Flyway,
  roles por organización, assignments, `@PreAuthorize` database-authoritative.
- **Branch Memberships** — usuarios por sucursal con roles branch-scoped y
  `hasBranchPermission` (org-wide UNION membership ACTIVE de esa branch).
- **Audit Foundation** — historial append-only (`audit_events`) con trigger DB,
  actor USER/SYSTEM, metadata JSONB segura y lectura paginada (`audit.read`).
- **Secure Admin Bootstrap** — provisión one-shot y transaccional de la primera
  Organization + primer administrador, gated por `BOOTSTRAP_SECRET`.

Detalle: `docs/01-architecture/modules.md`, `docs/04-security/security-model.md`
y `docs/04-security/secure-bootstrap.md`. El roadmap completo en
`docs/00-product/roadmap.md`.

## Filosofía del producto

- Producto comercial y reutilizable de HiQNet, no un proyecto open source genérico.
- Evolución prevista: múltiples sucursales → múltiples cajas → inventario → recetas →
  compras → reportes → hardware POS → administración y licenciamiento.
- Modelo de datos escalable por **Organization → Branch** desde el diseño inicial:
  los catálogos (productos, ingredientes) pertenecen a la organización; las
  **existencias pertenecen a la sucursal**.
- Simplicidad, modularidad, seguridad, testabilidad y cero sobreingeniería.

## Arquitectura general

**Modular Monolith** (ver `docs/01-architecture/architecture.md` y ADR-0001):

```text
backend/   Spring Boot API (REST, JSON) — Java 21, Maven, Spring Security,
           Spring Data JPA, Flyway, PostgreSQL, Actuator, Testcontainers
frontend/  React SPA — TypeScript estricto, Vite, React Router, TanStack Query, MUI
docs/      Documentación viva: producto, arquitectura (ADR), base de datos,
           API, seguridad, desarrollo
deploy/    Estrategia de despliegue (documentación por ahora)
```

No hay microservicios. No hay SaaS/multi-tenant todavía (ver `docs/01-architecture/architecture.md`).

## Stack y versiones

| Capa | Tecnología | Versión |
| --- | --- | --- |
| Backend | Java (Temurin) | 21 (LTS) |
| Backend | Spring Boot | 4.1.1 |
| Backend | Apache Maven (wrapper) | 3.9.16 |
| Backend | PostgreSQL | 17 |
| Backend | Flyway | gestionado por Spring Boot |
| Backend | Testcontainers | gestionado por Spring Boot |
| Frontend | Node.js | 24 LTS (referencia CI; mínimo soportado `>=22.12`, ver `engines`) |
| Frontend | package manager | pnpm 11.28.5 (autoridad: `packageManager` en `package.json`; lockfile versionado) |
| Frontend | React | 19 |
| Frontend | TypeScript | 6 (estricto) |
| Frontend | Vite | 8 |
| Frontend | React Router | 7 |
| Frontend | TanStack Query | 5 |
| Frontend | MUI | 9 |

Mantenemos **Java 21** y versiones estables; no seguimos la última versión por moda.

## Estructura del repositorio

```text
point-of-sale/
├── backend/            API Spring Boot (módulo único por ahora)
├── frontend/           SPA React + TypeScript + Vite
├── docs/               Documentación (producto, arquitectura, API, seguridad, dev)
│   ├── 00-product/
│   ├── 01-architecture/  (incluye adr/)
│   ├── 02-database/
│   ├── 03-api/
│   ├── 04-security/
│   └── 05-development/
├── deploy/             Estrategia de despliegue (documentación)
├── .github/workflows/  CI (backend + frontend + docker-compose)
├── docker-compose.yml  PostgreSQL local de desarrollo
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
└── VERSION             0.3.0-SNAPSHOT
```

## Requisitos

- Java 21 (Temurin/OpenJDK) — `java -version`
- Docker + Docker Compose — `docker --version`
- Node.js 24 LTS — `node --version`
- pnpm 11 — `pnpm --version` (o activa Corepack: `corepack enable`)

## Instalación y desarrollo local

```bash
git clone https://github.com/HiQNet-Dev/point-of-sale.git
cd point-of-sale

# 1. Levantar PostgreSQL (solo desarrollo; credenciales locales no productivas)
docker compose up -d
docker compose ps          # esperar a que el healthcheck pase

# 2. Backend (usa el wrapper; descarga Maven en la primera ejecución)
cd backend
./mvnw spring-boot:run     # Windows: .\mvnw.cmd
# Health check: http://localhost:8080/actuator/health → {"status":"UP"}
# (perfil dev por defecto; SPRING_PROFILES_ACTIVE=prod exige variables de entorno)

# 3. Frontend
cd ../frontend
pnpm install
pnpm dev                   # http://localhost:5173
```

Variables de entorno del backend (nombres seguros de ejemplo en `.env.example` y
`frontend/.env.example`):

| Variable | Uso | Default dev |
| --- | --- | --- |
| `DB_HOST`, `DB_PORT`, `DB_NAME` | Conexión PostgreSQL | `localhost`, `5432`, `hiqnet_pos` |
| `DB_USER` | Usuario PostgreSQL | `hiqnet_dev` |
| `DB_PASSWORD` | Contraseña PostgreSQL (*solo dev*) | `hiqnet_dev_only` |
| `SERVER_PORT` | Puerto HTTP del backend | `8080` |
| `CORS_ALLOWED_ORIGINS` | Orígenes CORS permitidos (lista, sin wildcard) | `http://localhost:5173` (dev) |
| `AUTH_JWT_SECRET` | Secreto de firma JWT (clave **Base64**, ≥32 bytes decodificados) — **obligatorio, sin default** | *(generar: `openssl rand -base64 32`)* |
| `BOOTSTRAP_SECRET` | Secreto one-shot del primer provisioning (`POST /api/v1/bootstrap`; Base64 ≥32 bytes). **Opcional**: ausente → la ruta queda cerrada. **Temporal**: retirar tras el primer bootstrap y reiniciar. Distinto de `AUTH_JWT_SECRET` | *(generar: `openssl rand -base64 32`; no dejar valor por defecto)* |
| `SPRING_PROFILES_ACTIVE` | Perfil Spring (`dev`/`prod`) | `dev` |

**Perfil `prod`** (`SPRING_PROFILES_ACTIVE=prod`): exige `DB_URL`, `DB_USER`,
`DB_PASSWORD` y `AUTH_JWT_SECRET` (sin defaults; fail-fast), OpenAPI cerrado,
CORS vacío salvo configuración explícita y `ddl-auto=validate` — Flyway es la
única autoridad de schema. Verificado por `FoundationProdProfileTest`.

**Primer arranque**: con `BOOTSTRAP_SECRET` configurado hacer
`POST /api/v1/bootstrap` (header `X-Bootstrap-Token`), luego quitar el secret,
reiniciar y entrar por `POST /api/v1/auth/login`. Detalle y operación:
`docs/04-security/secure-bootstrap.md`.

Frontend: `VITE_API_BASE_URL` (default `"/api/v1"`, same-origin).

## Comandos de verificación

```bash
# Backend: compila, corre pruebas (Testcontainers levanta PostgreSQL real) y empaqueta
cd backend && ./mvnw clean verify

# Frontend
cd frontend
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

CI ejecuta exactamente estos pasos en cada push a `main` y en cada PR.

## Roadmap (orientativo)

```text
v0.1.x Foundation      ✓ RELEASED (privada, 2026-10-10)
v0.2.x POS Core        ✓ RELEASED (privada, 2026-10-12)
v0.3.x Inventory       ⬜ IN DEVELOPMENT (Items/Stock/Recipe/Effects UI implemented; negative-stock UI pending)
v0.4.x Purchasing
v0.5.x Expenses & Management
v0.6.x Reporting
v0.7.x Hardware (Electron: impresora térmica, cajón, lectores)
v0.8.x Pilot
v0.9.x Release Candidate
v1.0.0 Commercial Release
```

Detalle en `docs/00-product/roadmap.md` y `docs/00-product/scope-v1.md`.

## Documentación

El directorio `docs/` es la fuente de verdad viva del proyecto:

- `docs/00-product/` — visión de producto, alcance v1, roadmap.
- `docs/01-architecture/` — arquitectura (modular monolith), módulos, ADRs.
- `docs/02-database/` — modelo de dominio conceptual, convenciones Flyway.
- `docs/03-api/` — convenciones de API.
- `docs/04-security/` — modelo de seguridad.
- `docs/05-development/` — guías de código, git workflow, release process.

## Cómo contribuir

Ver `CONTRIBUTING.md`: ramas `feat/*`, `fix/*`, `refactor/*`, `docs/*`,
Conventional Commits, CI obligatoria antes de merge.

## Seguridad

Reporta vulnerabilidades según `SECURITY.md`. Principios de seguridad en
`docs/04-security/security-model.md` (deny by default, least privilege, secretos
por entorno, autorización server-side).

## Licencia y propiedad intelectual

HiQNet POS es software propietario de HiQNet.

Copyright © 2026 HiQNet. All rights reserved.

Este repositorio y su código fuente no se publican bajo una licencia open-source.
No se concede permiso general para usar, copiar, modificar, redistribuir,
sublicenciar o crear trabajos derivados.

Los derechos de uso para clientes, instalaciones o terceros se otorgarán
únicamente mediante acuerdos comerciales separados con HiQNet.

Consulta `LICENSE` y `docs/05-development/legal-status.md`.

La release `v0.1.0` representa únicamente la Foundation técnica del producto;
no significa que el POS comercial completo esté terminado.