# Estrategia de CI

Documentación de cómo GitHub Actions valida el repositorio. Hay **dos niveles**:

- **CI general** — para pull requests y para pushes a ramas de trabajo y a
  `dev`. Verifica código limpio e integración de código: backend, frontend y
  compose. **No** ejecuta la integración E2E.
- **CI de `main`** — más robusto y completo. Se ejecuta al hacer push a `main`
  (y en la validación manual) y añade la integración E2E real (Playwright +
  PostgreSQL) además de lo anterior.

| Contexto | Backend | Frontend | Compose | E2E |
| --- | --- | --- | --- | --- |
| Pull request (hacia `dev` o `main`) | si aplica | si aplica | si aplica | no |
| Push a `dev` o ramas de trabajo | si aplica | si aplica | si aplica | no |
| Push a `main` | sí | sí | sí | **sí** |
| Manual (`workflow_dispatch`) | sí | sí | sí | **sí** |

No se eliminan ni se marcan como `skip` pruebas: la suite completa sigue
existiendo y se ejecuta en el nivel que corresponde.

## Detección de cambios

El job `Detect Changes` (`.github/scripts/ci-changes.sh`, solo `git`, sin actions
de terceros) clasifica los archivos modificados y publica los flags que consumen
los demás jobs:

| Área | Rutas (aproximadas) |
| --- | --- |
| `backend` | `backend/**` |
| `frontend` | `frontend/**` |
| docker | `docker-compose.yml`, `Dockerfile*`, `.dockerignore`, `.env.example`, `deploy/**` |
| ci | `.github/**` |

Reglas de decisión:

- `docs`-only → no ejecuta jobs ejecutables (solo `Detect Changes` + `CI Gate`).
- cambio de `backend`, `frontend`, `docker` o `.github/**` → ejecuta el job afín.
- `workflow_dispatch` → validación completa.
- sin base de diff válida (por ejemplo, primer push) → se cae a validación completa.
- ruta desconocida (no documentación y no mapeada a un área) → fail-safe:
  validación completa, para no omitir jobs ante código no reconocido.

La integración E2E **no** depende de la detección de cambios: corre solo en el CI
robusto de `main` y en la validación manual.

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
- **Playwright**: no se cachean los browsers; solo el CI de `main` instala
  Chromium, así que no compensa una cache frágil.

La versión de pnpm la fija `packageManager` en `frontend/package.json`
(`pnpm/action-setup@v4`); no se declara una versión distinta en el workflow.

## CI Gate

El job `CI Gate` depende de `changes`, `backend`, `frontend`, `compose` y `e2e`,
se ejecuta siempre (`if: always()`) y **falla si algún job requerido falló o se
canceló**; trata `skipped` como aceptable (los jobs que no aplican se saltan).
Es el único *required status check* recomendado en la protección de rama, lo que
evita depender de checks condicionales que a veces no existen.

## Validación manual

Desde la pestaña Actions, `workflow_dispatch` ejecuta la validación completa
(incluido el E2E) sobre un `target_sha` explícito (40 caracteres, desde `main`).
También puede lanzarse desde la CLI:

```bash
gh workflow run ci.yml --ref main -f target_sha=<commit-sha>
```

## Notas

- El servicio PostgreSQL es siempre `postgres:17` aislado y descartable
  (`hiqnet_pos_e2e`), nunca el volumen de desarrollo.
- Los artefactos de fallo (traces, screenshots, video, logs) se suben solo con
  `if: failure()`, con 7 días de retención.
