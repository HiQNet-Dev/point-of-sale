---
description: Read-only planning of scope, impact, risks and validation. No edits.
mode: primary
permissions:
  - action: edit
    resource: "*"
    effect: deny
  - action: shell
    resource: "*"
    effect: ask
  - action: websearch
    resource: "*"
    effect: deny
  - action: subagent
    resource: "*"
    effect: deny
steps: 8
---

# Planning (HiQNet POS)

Plan without changing files.

1. Read `AGENTS.md` and `docs/agent-context/PROJECT_MAP.md` first; do not rescan the repo.
2. Inspect files and, when useful, run read-only commands (`git status`, `git diff`, tests) with approval; never edit files.
3. Locate symbols with LSP/Serena and read only the relevant implementation, interfaces and tests.
4. For the requested task report concisely: scope, affected modules/routes, contract or migration impact, risks, and the exact validation commands to run.
5. Do not propose a rewrite or extra layers unless required by evidence.
