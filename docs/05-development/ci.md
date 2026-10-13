# Estrategia de CI

Documentación de cómo GitHub Actions valida el repositorio. El objetivo es dar
feedback rápido en los pull request sin perder cobertura: la misma batería de
tests sigue existiendo, solo cambia **qué** se ejecuta según el área modificada.
Cada job se ejecuta **una sola vez** (sin bucles ni repeticiones).

## Modelo de ejecución

| Contexto | Validación |
| --- | --- |
| Pull request | Detección de cambios → solo los jobs de las áreas modificadas + **1** E2E si aplica |
| Push a `main` | Validación completa (Backend + Frontend + Compose + **1** E2E) |
| Manual (`workflow_dispatch`) | Validación completa con un `target_sha` explícito |

No se eliminan ni se marcan como `skip` pruebas: la suite completa sigue
existiendo y ejecutándose cuando corresponde.

## Detección de cambios

El job `Detect Changes` (`.github/scripts/ci-changes.sh`, solo `git`, sin actions
de terceros) clasifica los archivos modificados y publica los flags que consumen
los demás jobs:

| Área | Rutas (aproximadas) |
| --- | --- |
| `backend` | `backend/**` |
| `frontend` | `frontend/**` |
| frontend de producción | `frontend/**` excluyendo `frontend/e2e/**`, `frontend/scripts/e2e/**`, `playwright.config.*`, `tsconfig.e2e.json` y `*.test.ts(x)` |
| harness E2E | `frontend/e2e/**`, `frontend/scripts/e2e/**`, `frontend/playwright.config.*`, `frontend/tsconfig.e2e.json` |
| docker | `docker-compose.yml`, `Dockerfile*`, `.dockerignore`, `.env.example`, `deploy/**` |
| ci | `.github/**` |

Reglas de decisión:

- `docs` / solo `*.test.ts(x)` → no ejecuta E2E.
- `backend`, frontend de producción, harness E2E o `.github/**` → ejecuta **1** E2E.
- `schedule` / `workflow_dispatch` → validación completa.
- sin base de diff válida (por ejemplo, primer push) → se cae a validación completa.

## Concurrency

El workflow cancela runs obsoletos cuando llega un commit nuevo al mismo PR o a
la misma rama/evento:

```yaml
concurrency:
  group: ci-${{ github.workflow }}-${{ github.event_name }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
```

Así, un commit nuevo en el mismo PR cancela el run obsoleto, sin cancelar entre
sí runs independientes de `main` o `workflow_dispatch` (el grupo incluye el
evento).

## Caches

- **Maven**: `actions/setup-java@v4` con `cache: maven` (invalida con `pom.xml`).
- **pnpm**: `actions/setup-node@v4` con `cache: pnpm` y
  `cache-dependency-path: frontend/pnpm-lock.yaml`.
- **Playwright**: no se cachean los browsers. La instalación de Chromium ronda
  los 20-25 s y solo ocurre en el job E2E; no compensa una cache frágil.

La versión de pnpm la fija `packageManager` en `frontend/package.json`
(`pnpm/action-setup@v4`); no se declara una versión distinta en el workflow.

## CI Gate

El job `CI Gate` depende de `changes`, `backend`, `frontend`, `compose` y `e2e`,
se ejecuta siempre (`if: always()`) y **falla si algún job requerido falló o se
canceló**; trata `skipped` como aceptable (los jobs condicionales que no aplican
se saltan). Es el único *required status check* recomendado en la protección de
rama, lo que evita depender de checks condicionales que a veces no existen.

## Validación manual

Desde la pestaña Actions, `workflow_dispatch` permite validar un `target_sha`
explícito (40 caracteres, desde `main`) con la batería completa. También puede
lanzarse desde la CLI:

```bash
gh workflow run ci.yml --ref main -f target_sha=<commit-sha>
```

## Notas

- El servicio PostgreSQL es siempre `postgres:17` aislado y descartable
  (`hiqnet_pos_e2e`), nunca el volumen de desarrollo.
- Los artefactos de fallo (traces, screenshots, video, logs) se suben solo con
  `if: failure()`, con 7 días de retención.
