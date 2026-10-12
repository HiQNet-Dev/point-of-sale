# POS integrated E2E harness

## Purpose and scope

The Chromium suite exercises the real browser application against the real Spring
Boot API and a disposable PostgreSQL 17 database. It is verification
infrastructure, not a business-domain module. The initial journey covers Secure
Admin Bootstrap, API-created test fixtures, cashier login/session recovery,
Branch/Register/Shift setup, explicit Sale creation, Product configuration,
persisted cart mutations, CASH/CARD/split/zero-total checkout and Shift close.

The E2E cashier role is constructed from the live permission catalog. Its exact
permissions are `cash-register.read`, `cash-shift.read`, `cash-shift.open`,
`cash-shift.close`, `pos.catalog.read`, `sale.read`, `sale.create`,
`sale.update`, `sale.cancel` and `sale.checkout`. It deliberately omits
`payment.read`; the browser journey asserts that checkout makes no payment-history
request. All users, credentials, Products and transactions are synthetic.

## Architecture and safety

- GitHub Actions provisions an isolated PostgreSQL 17 service for one job. Its
  database and test-only credentials are discarded with the job.
- `application-e2e.yml` only reads `E2E_DB_URL`, `E2E_DB_USER` and
  `E2E_DB_PASSWORD`; it does not contain credentials or a development fallback.
- Before startup, `scripts/e2e/safety-check.mjs` requires `APP_ENV=e2e`, an
  explicit PostgreSQL JDBC URL, a loopback host and a database name beginning
  `hiqnet_pos_e2e`. Do not use this profile against production, staging or a
  shared development database.
- Playwright global setup waits for `/actuator/health`, performs the one-shot
  Secure Admin Bootstrap using the generated `BOOTSTRAP_SECRET`, and creates
  Branch/Register/users/roles/catalog fixtures through public APIs. It does not
  insert business data with SQL.
- The browser tests use actual UI flows. Setup secrets and tokens are not written
  into a browser storage state; the temporary fixture file contains only
  synthetic identity/resource IDs and is written under the operating-system temp
  directory with owner-only permissions where supported.
- `assert-db.sql` is read-only and fails the CI job if exact Sale, Payment,
  snapshot or audit evidence is absent.
- Chromium uses one worker, no retry, retain-on-failure traces/video/screenshots
  and an HTML report. CI uploads the failure artifacts and backend/frontend logs
  for seven days.

## Local prerequisites and commands

A local run requires Java 21, Node 24, pnpm, Chromium and a dedicated disposable
PostgreSQL instance. Docker Desktop being installed is not sufficient; its engine
must be running. Use a database named `hiqnet_pos_e2e` (never the ordinary
`hiqnet_pos` development database). The E2E profile will refuse remote database
hosts and other database names.

Set these environment values in the local shell; use synthetic passwords and
random secrets, never production credentials:

```text
APP_ENV=e2e
SPRING_PROFILES_ACTIVE=e2e
E2E_DB_URL=jdbc:postgresql://127.0.0.1:5432/hiqnet_pos_e2e
E2E_DB_USER=<local disposable DB user>
E2E_DB_PASSWORD=<local disposable DB password>
BOOTSTRAP_SECRET=<base64 encoding of at least 32 random bytes>
AUTH_JWT_SECRET=<base64 encoding of at least 32 random bytes>
E2E_ADMIN_PASSWORD=<synthetic password, 12-128 chars>
E2E_CASHIER_PASSWORD=<synthetic password, 12-128 chars>
```

Then run the safety check and the app processes in separate terminals:

```text
node frontend/scripts/e2e/safety-check.mjs
cd backend && ./mvnw spring-boot:run
cd frontend && pnpm install --frozen-lockfile
cd frontend && pnpm exec playwright install chromium
cd frontend && VITE_API_BASE_URL=http://127.0.0.1:8080/api/v1 pnpm dev --host 127.0.0.1 --port 5173
cd frontend && pnpm e2e
```

Set `E2E_API_URL=http://127.0.0.1:8080/api/v1`,
`E2E_FRONTEND_URL=http://127.0.0.1:5173`,
`CORS_ALLOWED_ORIGINS=http://127.0.0.1:5173` and optionally
`E2E_ORGANIZATION_CODE=pos-e2e`; these are the defaults used by the harness.
The bootstrap endpoint is one-shot and creates historical records. Always start
from a freshly created disposable database; do not point repeated runs at an old
fixture database. Run headed mode with `pnpm e2e:headed`. For read-only local
persistence evidence, run `psql` against that same dedicated DB with
`-v org_code=pos-e2e -f frontend/scripts/e2e/assert-db.sql` after the browser run.

## CI and troubleshooting

CI runs three sequential, independent `POS integrated E2E (Chromium + PostgreSQL)`
matrix executions, each with a fresh isolated PostgreSQL service and one-shot
bootstrap. This supplies three clean-state repetitions without retries or
historical fixture reuse. Each run migrates through Spring Boot startup, waits by
polling health/HTTP endpoints (bounded timeout),
starts Vite with an explicit backend API URL, runs Playwright, then runs the
read-only SQL assertions. CI-generated bootstrap/JWT/user passwords are random, masked with GitHub Actions
`add-mask`, and appended to the Actions environment without printing their values.

A setup/API failure is a harness/contract setup failure until the response and
backend log establish otherwise. A browser assertion failure against a successful
real API response may indicate a product defect; preserve the Playwright trace,
screenshot and server logs, and do not soften the expected behavior to hide it.
`pnpm e2e` requires a fresh empty E2E database because bootstrap is one-shot.
The local environment used to create this harness did not have a running Docker
Engine or `psql`; local integrated execution therefore requires enabling those
prerequisites. CI is the reproducible execution environment.
