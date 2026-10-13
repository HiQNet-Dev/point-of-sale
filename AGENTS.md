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
