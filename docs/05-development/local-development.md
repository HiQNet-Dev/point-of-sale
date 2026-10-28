# Local development environment

Reproducible local setup for HiQNet POS. Docker provides PostgreSQL, the Spring
Boot backend and the Vite frontend; a hybrid mode keeps the fast frontend loop on
the host. This document is the single source of truth for local development;
[`README.md`](../../README.md) only carries a short quick start.

## Foundation Rust (incremento paralelo, no sustitución)

El entorno descrito abajo sigue siendo el **AS-IS Java/React oficial**. La nueva
foundation está en [`backend-rust/README.md`](../../backend-rust/README.md):
workspace Rust, SQLite sintético y migration-lab, sin UI nueva ni API compatible.
No conectar React a Rust ni ejecutar importaciones reales. Sus raíces SQLx son
independientes de Flyway y su gate normativo permanece bloqueado. Ver
[`rust-foundation.md`](rust-foundation.md) para evidencia y límites.

## Architecture

```text
Host browser
  │  http://localhost:5173          http://localhost:8080/api/v1
  ▼
frontend container (Vite dev server + HMR)  ──HTTP──▶  backend container (Spring Boot)
                                                              │
                                                              ▼
                                                   db container (PostgreSQL 17, Flyway V1–V25)
```

Everything runs inside the Compose network; only the published ports are
reachable from the host. The browser always talks to the **host** URLs above, so
the frontend uses `VITE_API_BASE_URL=http://localhost:8080/api/v1` (never the
Compose service name, which only resolves inside the network).

## Prerequisites

- Full Docker mode: **Git** + **Docker** with the Compose plugin.
- Hybrid mode additionally: **Node.js 24 LTS** (`>=22.12`, see `frontend/package.json`
  `engines`) and **pnpm** (`corepack enable`).
- You do **not** need Java, Maven or PostgreSQL on the host.

## Quick start (full Docker)

```bash
git clone https://github.com/HiQNet-Dev/point-of-sale.git
cd point-of-sale
cp .env.example .env                 # development-only defaults
docker compose up --build            # db + backend + frontend
```

Wait until the backend is healthy (the frontend starts once the backend reports
`UP`), then load the fixtures and sign in:

```bash
docker compose --profile tools run --rm seed     # load the development dataset
```

Open <http://localhost:5173> and sign in with a development account (below).

## Development modes

### Mode A — Full Docker

```bash
docker compose up --build     # db + backend + frontend
docker compose down           # stop (keeps data)
```

### Mode B — Docker DB + backend, local Vite (recommended for frontend work)

```bash
docker compose up -d db backend       # PostgreSQL + Spring Boot in Docker
docker compose --profile tools run --rm seed   # fixtures (once)
cd frontend
pnpm install
pnpm dev                               # host Vite on http://localhost:5173
```

Faster HMR, no Vite container overhead. The backend and DB still come from Docker.

### Mode C — Database only

```bash
docker compose up -d db      # just PostgreSQL, for a backend run on the host
# then: cd backend && ./mvnw spring-boot:run   (profile dev by default)
```

### Existing automated tests (unchanged)

```bash
cd backend && ./mvnw clean verify     # Testcontainers starts its own PostgreSQL
cd frontend && pnpm lint && pnpm typecheck && pnpm test && pnpm build
cd frontend && pnpm e2e               # Playwright E2E (see the E2E section below)
```

These never use the development Compose stack or its volume.

## Service URLs and ports

| Service | URL / port | Notes |
| --- | --- | --- |
| Frontend | <http://localhost:5173> | Vite dev server (`VITE_PORT`) |
| Backend | <http://localhost:8080> | Spring Boot (`SERVER_PORT`) |
| Backend health | <http://localhost:8080/actuator/health> | `{"status":"UP"}` when ready |
| Backend OpenAPI | <http://localhost:8080/swagger-ui/index.html> | dev profile only |
| PostgreSQL | `localhost:5432` (`POSTGRES_PORT`) | published for host tools/backend |

## Environment variables

All local values live in a root `.env` (copy of `.env.example`, gitignored).
Compose interpolates it for containers; the seed/reset scripts load it too; and
the host Vite dev server reads it (`envDir` points at the repository root), so
`pnpm dev` picks up `VITE_API_BASE_URL` / `VITE_USE_POLLING` from the same file
without a separate `frontend/.env`.

| Variable | Purpose | Development default |
| --- | --- | --- |
| `APP_ENV` | Safety guard for `dev:seed`/`dev:reset` | `development` |
| `POSTGRES_DB` / `POSTGRES_USER` / `POSTGRES_PASSWORD` | DB name/user/password | `hiqnet_pos` / `hiqnet_dev` / `hiqnet_dev_only` |
| `POSTGRES_PORT` / `SERVER_PORT` / `VITE_PORT` | Published ports | `5432` / `8080` / `5173` |
| `AUTH_JWT_SECRET` | JWT signing key (Base64, ≥32 bytes). Required by the backend | development-only value |
| `BOOTSTRAP_SECRET` | One-shot provisioning secret for the seed | development-only value |
| `CORS_ALLOWED_ORIGINS` | Backend CORS allowlist | `http://localhost:5173` |
| `VITE_API_BASE_URL` | Browser-facing backend URL | `http://localhost:8080/api/v1` |
| `VITE_USE_POLLING` | Vite file-watch polling | `false` |
| `DEV_*` | Development fixture inputs (see the seed) | fictitious values |

Frontend `VITE_*` variables are exposed to the browser and must never hold secrets.

## Development fixtures

Loaded by `pnpm dev:seed` (host) or `docker compose --profile tools run --rm seed`
(Docker) through the real bootstrap + administrative APIs, so every row respects
tenant/branch ownership, RBAC and versioning. The dataset is 100 % fictitious.

| Fixture | Value |
| --- | --- |
| Organization | `HiQNet POS Development` (`pos-dev`), MXN, `es-MX` |
| Branches | `Sucursal Centro` (`dev-a`), `Sucursal Norte` (`dev-b`) |
| Cash registers | `dev-a1`, `dev-a2` (Centro); `dev-b1` (Norte) |
| Categories | `dev-coffee` (Café), `dev-tea` (Té) |
| Products | `DEV-AMERICANO`, `DEV-LATTE` (tracked), `DEV-TEA` |
| Inventory items | `dev-beans` (g), `dev-milk` (ml), `dev-cups` (unit) |
| Opening stock | Centro: 5000 g / 5000 ml / 100 u; Norte: 2000 g / 2000 ml / 50 u |
| Recipes | Americano → 18 g beans; Latte → 18 g beans + 200 ml milk + 1 cup |
| Modifier group | `dev-size` (Tamaño): Regular, Grande (+10.00, milk effect +100) |
| Roles | `dev-inventory-admin` (inventory perms), `dev-cashier` (POS perms) |

### Development credentials — DEVELOPMENT ONLY

| Account | Email | Password | Use |
| --- | --- | --- | --- |
| Administrator | `admin@hiqnet.local` | `admin-dev-only` | full organization access |
| Inventory manager | `manager@hiqnet.local` | `staff-dev-only` | Inventory admin UI (`/admin`) |
| Cashier | `cashier@hiqnet.local` | `staff-dev-only` | POS (`/pos`) in both branches |

> These accounts and passwords are intentionally insecure fixtures. **Never use
> them for real customers, and never reuse these values in production.**

### Tracked sale

`DEV-LATTE` has an ACTIVE recipe and modifier effect, and both branches have
opening stock, so a checkout consumes inventory. Opening a sale requires an OPEN
cash shift: in `/pos` select a branch and register and open a shift first (a
normal operational step, not seeded).

## Database lifecycle

- **First start:** the `db` volume is created empty; Flyway (driven by the
  backend, `V1–V21`) creates the schema on backend start.
- **Persistence:** `docker compose down` and `docker compose up` keep the data in
  the named volume.
- **Seed:** run once against a pristine database (`pnpm dev:seed`). It is not
  idempotent — the provisioning bootstrap only succeeds on a fresh database.
- **Reset (deterministic):** `pnpm dev:reset` drops and recreates **only** the
  development database, restarts the backend so Flyway re-applies the migrations,
  and re-runs the seed. Use `pnpm dev:reset -- --no-seed` to reset without seeding.
- **Destroy:** `docker compose down -v` removes the volume and **all** local data.
  This is destructive and is not part of the daily flow.

### Reset safety

`dev:seed` and `dev:reset` refuse to run unless `APP_ENV=development`, the
database is named exactly `hiqnet_pos`, the host is loopback or the Compose
service, and `SPRING_PROFILES_ACTIVE` is not `prod`. There is no generic
"drop `$DATABASE_URL`" path. The development database is separate from the E2E
database (`hiqnet_pos_e2e*`) used by CI and Playwright.

## HMR

Edit any file under `frontend/src`, save, and the browser updates without a
rebuild. Verify by changing a visible string and watching the page refresh.

On Windows + Docker Desktop, bind-mounted file events may not reach the
container. If HMR stops detecting changes set `VITE_USE_POLLING=true` in `.env`
and restart the frontend container. Polling is opt-in to keep the host watcher
cheap on Linux/macOS.

## Logs and health

```bash
docker compose logs -f backend     # backend
docker compose logs -f frontend    # Vite
docker compose logs -f db          # PostgreSQL
docker compose ps                  # status + health
curl http://localhost:8080/actuator/health   # backend readiness
```

## Troubleshooting

- **Port already in use:** change `POSTGRES_PORT`, `SERVER_PORT` or `VITE_PORT` in
  `.env` and re-run. If you change `VITE_PORT`, update `CORS_ALLOWED_ORIGINS` too.
- **Database unhealthy / backend waiting:** `docker compose ps`, then
  `docker compose logs db`. The backend waits for `db` to be healthy.
- **HMR not detecting files:** see the HMR section (`VITE_USE_POLLING=true`).
- **Stale data:** `pnpm dev:reset` (deterministic reset) rather than deleting the
  volume.
- **Stale images:** `docker compose up --build` (or `docker compose build`).
- **Seed refuses to run:** it needs a pristine DB — run `pnpm dev:reset`.

## Windows notes

Docker Desktop bind mounts may not propagate filesystem events; enable
`VITE_USE_POLLING=true`. Run the commands from PowerShell or Git Bash — the dev
scripts are plain Node and have no Bash requirement. The backend Maven wrapper is
contained in the image, so no host Maven/Java is needed.

## Security warning

The development credentials, seed data and reset commands are for local use only.
They must never be used against real customer data, and the reset commands must
never be pointed at anything other than a local `hiqnet_pos` database.
