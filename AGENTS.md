# AGENTS.md — HiQNet POS repository rules for agents

Repo-local rules for automated agents working on this repository. They
complement the human workflow in `docs/05-development/git-workflow.md` and
`CONTRIBUTING.md`.

## Branching

- Always branch from the current `origin/main`; never work directly on `main`.
- Use short-lived branches with an approach prefix
  (`feat/backend/*`, `feat/frontend/*`, `feat/fullstack/*`, `fix/*`,
  `chore/dev/*`, `chore/ci/*`, `docs/*`, `test/*`).
- Never create long-lived parallel branches such as `backend`, `frontend`,
  `develop`, `docker` or `integration`; `main` is the single canonical branch
  and must always stay integrated (backend + frontend + Flyway + Docker + docs +
  green CI).

## Worktrees

- Keep **at most one active feature worktree** at a time, unless there is an
  explicitly documented reason (e.g. two open PRs).
- After a PR is merged: remove its worktree, delete the local branch, and
  `git fetch` to update `main`.
- Never commit `.worktrees/` (it is local and git-ignored).

## Pull requests

- One coherent purpose per PR (do not mix Docker, UI features, schema
  migrations and unrelated cleanup).
- Do not commit, push to `main`, force-push, or rewrite published history
  unless explicitly requested.
- Never commit secrets or real `.env` files.
