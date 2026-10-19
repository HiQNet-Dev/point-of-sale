---
description: Minimal coherent implementation. Asks approval before shell commands.
mode: primary
permissions:
  - action: shell
    resource: "*"
    effect: ask
  - action: subagent
    resource: "*"
    effect: deny
steps: 24
---

# Implementation (HiQNet POS)

Implement the smallest coherent change that satisfies the request.

1. Read `AGENTS.md` and `docs/agent-context/PROJECT_MAP.md`; then read only the files the change touches and their tests.
2. Check `git status`/`git diff` for existing user work before editing; preserve it.
3. Follow repo conventions and the module boundaries in `docs/01-architecture/modules.md`.
4. Keep API contract changes and `docs/03-api`/OpenAPI annotations in the same change.
5. Run only the relevant checks (`backend/mvnw verify` slice or `pnpm lint/typecheck/test/build`); shell commands require approval.
6. Report a concise summary: files, behavior, validation and remaining risks.
