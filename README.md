# HiQNet POS

Sistema punto de venta modular de [HiQNet](https://github.com/HiQNet-Dev), orientado
principalmente a **pequeños negocios de alimentos** (cafeterías, juguerías, restaurantes
pequeños) y **retail ligero**.

> **Estado:** HiQNet POS está en desarrollo activo y todavía **no es una release
> comercial estable**. Este repositorio contiene únicamente la fundación del producto:
> structure técnica compilable, ejecutable y documentada. Las funcionalidades del POS
> aún no existen.

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
| Frontend | Node.js | 24 LTS |
| Frontend | package manager | pnpm 11.24.0 (lockfile versionado) |
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
└── VERSION             0.1.0-SNAPSHOT
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
| `SPRING_PROFILES_ACTIVE` | Perfil Spring (`dev`/`prod`) | `dev` |

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
v0.1.x Foundation      ← estamos aquí
v0.2.x POS Core
v0.3.x Inventory
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

## Licencia

**Pendiente de definición.** Este es un producto comercial de HiQNet; no se concede
ninguna licencia de uso por defecto sobre el contenido de este repositorio. No
asumas que es open source ni reutilices su código fuera de HiQNet sin autorización.