# Flujo de trabajo con Git

## `main` es la única rama principal

- `main` es **canónica y estable**: siempre contiene backend + frontend +
  migraciones Flyway + Docker/Compose válido + docs + CI verde.
- **No se hace push directo a `main`** salvo casos justificados y comunicados.

**Excepción ya registrada:** el primer commit se hizo directo en `main` porque
el repositorio estaba vacío. Mensaje: `chore: initialize HiQNet POS foundation`.

### No hay ramas principales paralelas

Prohibido mantener ramas de integración de vida larga como `backend`,
`frontend`, `develop`, `docker` o `integration`. Generan drift respecto a
`main`. Backend y frontend viven juntos en `main` (monorepo); los enfoques se
separan con **prefijos de rama**, no con ramas permanentes.

## Ramas: por enfoque, pero short-lived

| Prefijo | Uso |
| --- | --- |
| `feat/backend/<feature>` | Funcionalidad de backend |
| `feat/frontend/<feature>` | Funcionalidad de frontend |
| `feat/fullstack/<feature>` | Funcionalidad que cruza backend + frontend |
| `fix/backend/<bug>` | Corrección de backend |
| `fix/frontend/<bug>` | Corrección de frontend |
| `chore/dev/<task>` | Tooling de desarrollo (Docker, scripts, entorno) |
| `chore/ci/<task>` | CI / workflows |
| `docs/<task>` | Solo documentación |
| `test/<task>` | Pruebas |
| `refactor/<task>` | Reestructuración sin cambio de comportamiento |
| `chore/<task>` | Mantenimiento / release-prep (resto) |

Ejemplos: `feat/backend/purchasing-foundation`,
`feat/frontend/inventory-settings`, `feat/fullstack/supplier-management`,
`chore/dev/docker-backend-db`, `docs/local-development`.

Toda rama: **sale de `origin/main` actual**, tiene un único objetivo, abre PR,
se audita, se mergea y se elimina. Nunca se usa como rama de integración
permanente.

## Worktrees

- Política: **como máximo un worktree de feature activo** por desarrollador,
  salvo necesidad excepcional documentada (p. ej. dos PRs abiertos a la vez).
- Flujo: `main` limpio → `git fetch` → crear branch/worktree → implementar →
  PR → auditoría → merge → **eliminar worktree** → **eliminar branch local** →
  actualizar `main`.
- No acumular worktrees históricos: `.worktrees/` es local y está en
  `.gitignore`; nunca se commitea.

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
- **CI obligatoria antes de merge** (backend + frontend + docker-compose):
  exactamente `./mvnw clean verify` y `pnpm lint/typecheck/test/build`.
- **Squash merge** (convención actual). Después del merge:
  eliminar branch remoto, branch local y worktree.
- Tras cada merge: `git fetch origin` y actualizar `main` local cuando esté
  limpio. No dejar `main` local desactualizado.

## Higiene de Git

- `git diff --check` antes de commitear (sin whitespace basura).
- **No commitear archivos generados** (`target/`, `node_modules/`, `dist/`,
  `.worktrees/`, lockfiles ajenos al gestor previsto — `pnpm-lock.yaml` sí se
  versiona).
- **Nunca secretos**, credenciales ni `.env` reales en el historial.
- Diffs pequeños y enfocados; nada de cambios no relacionados en un PR.

## Reglas para agentes

- No crear un worktree nuevo si ya hay otro worktree de feature activo, salvo
  justificación explícita.
- Siempre ramificar desde `origin/main` actual.
- Después del merge: eliminar el worktree y el branch local mergeados.
- No dejar ramas `backend`/`frontend` permanentes ni worktrees históricos.
