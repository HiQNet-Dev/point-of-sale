# Flujo de trabajo con Git

## Ramas

- **`main`**: rama estable/release. Representa la última versión liberada. No se
  desarrolla directamente en `main`; solo la recibe `dev` durante una release
  aprobada, o `hotfix/*` para correcciones urgentes de producción.
- **`dev`**: rama permanente de integración y preproducción de la próxima
  release. Todas las features normales nacen de `dev` y terminan en `dev`.
- Las ramas de trabajo son **short-lived** y nacen de `dev`.

```text
main  (estable / release)
  ↑
dev   (integración / preproducción)
  ↑
feat/* · fix/* · test/* · chore/* · docs/* · refactor/*
```

**Excepción ya registrada:** el primer commit se hizo directo en `main` porque el
repositorio estaba vacío. Mensaje: `chore: initialize HiQNet POS foundation`.

### No hay ramas principales paralelas

Prohibido mantener ramas de integración de vida larga como `backend`,
`frontend`, `docker` o `integration`. Backend y frontend viven juntos en el
monorepo; los enfoques se separan con **prefijos de rama**, y la integración
ocurre en `dev`.

## Ramas: por enfoque, pero short-lived

| Prefijo | Uso | Base | PR hacia |
| --- | --- | --- | --- |
| `feat/backend/<feature>` | Funcionalidad de backend | `dev` | `dev` |
| `feat/frontend/<feature>` | Funcionalidad de frontend | `dev` | `dev` |
| `feat/fullstack/<feature>` | Funcionalidad que cruza backend + frontend | `dev` | `dev` |
| `fix/backend/<bug>`, `fix/frontend/<bug>` | Corrección de bugs | `dev` | `dev` |
| `chore/dev/<task>`, `chore/ci/<task>`, `chore/<task>` | Tooling / CI / mantenimiento | `dev` | `dev` |
| `docs/<task>` | Solo documentación | `dev` | `dev` |
| `test/<task>` | Pruebas | `dev` | `dev` |
| `refactor/<task>` | Reestructuración sin cambio de comportamiento | `dev` | `dev` |
| `hotfix/<bug>` | Corrección urgente de la versión estable | `main` | `main` |

Toda rama: **sale de `origin/dev` actual** (o de `origin/main` para `hotfix/*`),
tiene un único objetivo, abre PR, se audita, se mergea y se elimina. Nunca se usa
como rama de integración permanente.

## Flujo normal

```bash
git switch dev
git pull --ff-only

git switch -c feat/frontend/my-feature
# implementar, testear, commitear
git push -u origin feat/frontend/my-feature
```

Abre un PR **hacia `dev`**, se audita, se hace squash merge y se elimina la rama.
Después: `git switch dev && git pull --ff-only`.

## Release

```text
features → dev → Release Audit → PR dev → main → CI verde → tag vX.Y.Z
```

El merge `dev → main` ocurre solo tras la Release Audit aprobada, con CI verde.
Después, `dev` continúa para la siguiente versión.

## Hotfix

```text
main → hotfix/<bug> → PR → main
```

Tras el merge en `main`, sincroniza el fix hacia `dev` (merge o cherry-pick según
corresponda) para que la próxima release no lo pierda.

## Worktrees

- **Trabaja en el checkout principal** (`D:\Proyectos\HiQNet\point-of-sale`)
  usando `git switch` y ramas short-lived. Los worktrees **no son el flujo por
  defecto**.
- Crea un worktree **solo** cuando necesites mantener dos checkouts
  simultáneamente (p. ej. dos PRs activos) y **documenta la razón** en el PR.
- `.worktrees/` es local y está en `.gitignore`; nunca se commitea. No acumules
  worktrees históricos.

## Commits: Conventional Commits

```text
<tipo>: <descripción corta en imperativo>

feat: add branch stock endpoint
fix: correct payment rounding
chore: initialize HiQNet POS foundation
docs: document security model
```

Tipos usados: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `ci`, `build`.
Sin force-push sobre ramas compartidas; sin reescribir historia publicada.

## Pull Requests y merge

- **PR es el camino normal** de desarrollo, incluso para el autor principal.
- Un PR, un propósito coherente: no mezclar Docker + feature UI + migración +
  limpieza aleatoria salvo dependencia inevitable.
- **CI obligatoria antes del merge** (backend + frontend + docker-compose + E2E
  cuando aplique): exactamente `./mvnw clean verify` y
  `pnpm lint/typecheck/test/build`.
- **Squash merge** (convención actual). Después del merge: eliminar branch remoto,
  branch local y worktree.
- Destino del PR: features/fixes/docs → `dev`; `hotfix/*` → `main`.
- Tras cada merge: `git fetch origin` y actualizar la rama base local (`dev`)
  cuando esté limpia. No dejar `dev` local desactualizada.

## Higiene de Git

- `git diff --check` antes de commitear (sin whitespace basura).
- **No commitear archivos generados** (`target/`, `node_modules/`, `dist/`,
  `.worktrees/`, lockfiles ajenos al gestor previsto — `pnpm-lock.yaml` sí se
  versiona).
- **Nunca secretos**, credenciales ni `.env` reales en el historial.
- Diffs pequeños y enfocados; nada de cambios no relacionados en un PR.

## Reglas para agentes

- Trabaja en el checkout principal con `git switch`; **no uses worktrees por
  defecto** (solo con justificación explícita documentada).
- Ramifica desde `origin/dev` actual (o `origin/main` para un `hotfix/*`).
- Abre los PR hacia `dev`; `hotfix/*` hacia `main`.
- No desarrollar directamente sobre `main` ni `dev` (la transición de modelo y
  las tareas de release/sincronización controlada son la única excepción).
- Después del merge: eliminar el worktree (si se usó) y el branch mergeado.
- No dejar ramas `backend`/`frontend` permanentes ni worktrees históricos.
