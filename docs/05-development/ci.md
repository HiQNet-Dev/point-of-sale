# Estrategia de CI

Documentación de cómo GitHub Actions valida el repositorio. El objetivo es dar
feedback rápido en los pull request sin perder cobertura: la misma batería de
tests sigue existiendo, solo cambia **cuándo** se ejecutan las repeticiones
redundantes.

## Modelo de ejecución

| Contexto | Validación |
| --- | --- |
| Pull request | Detección de cambios → solo los jobs de las áreas modificadas + como máximo **1** E2E normal |
| Push a `main` | Validación completa (Backend + Frontend + Compose + **1** E2E) |
| Ejecución nocturna (`schedule`) | Validación completa + **E2E x3** para detectar flakiness |
| Manual (`workflow_dispatch`) | Validación completa + **E2E x3** (auditorías de release, refactors grandes, investigación de flakes) |

No se eliminan ni se marcan como `skip` pruebas: la suite completa sigue
existiendo y ejecutándose donde aporta valor.

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
- cambio de `backend` o de frontend de producción → E2E **x1**.
- cambio del harness E2E o de `.github/**` → E2E **x3** (estabilidad del harness y del propio pipeline).
- `schedule` / `workflow_dispatch` → validación completa con E2E **x3**.
- sin base de diff válida (por ejemplo, primer push) → se cae a validación completa.

El conteo de repeticiones se pasa al matrix del job E2E como JSON
(`["1"]`, `["1","2","3"]` o `[]`), de modo que un PR normal no repite E2E.

## Concurrency

El workflow cancela runs obsoletos cuando llega un commit nuevo al mismo PR o a
la misma rama/evento:

```yaml
concurrency:
  group: ci-${{ github.workflow }}-${{ github.event_name }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
```

Así, un commit nuevo en el mismo PR cancela el run obsoleto, sin cancelar entre
sí runs independientes de `main`, `schedule` o `workflow_dispatch` (el grupo
incluye el evento).

## Caches

- **Maven**: `actions/setup-java@v4` con `cache: maven` (invalida con `pom.xml`).
- **pnpm**: `actions/setup-node@v4` con `cache: pnpm` y
  `cache-dependency-path: frontend/pnpm-lock.yaml`.
- **Playwright**: no se cachean los browsers. La instalación de Chromium ronda
  los 20-25 s y solo ocurre en el job E2E; reducir el número de ejecuciones E2E
  aporta más que optimizar su instalación, y evita caches frágiles.

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
explícito (40 caracteres, desde `main`) con la batería completa y E2E x3. Se usa
para auditorías de release, refactors grandes, confianza pre-release e
investigación de flakiness. También puede lanzarse desde la CLI:

```bash
gh workflow run ci.yml --ref main -f target_sha=<commit-sha>
```

## Notas

- El servicio PostgreSQL es siempre `postgres:17` aislado y descartable
  (`hiqnet_pos_e2e`), nunca el volumen de desarrollo.
- Los artefactos de fallo (traces, screenshots, video, logs) se suben solo con
  `if: failure()`, con 7 días de retención.
- La ejecución nocturna corre a las 06:00 UTC.
